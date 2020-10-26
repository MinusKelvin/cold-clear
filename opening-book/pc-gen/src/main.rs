use opening_book::{ Book, BookBuilder, Position };
use libtetris::FallingPiece;
use std::sync::atomic::AtomicBool;
use std::collections::{ HashSet, HashMap };
use enumset::EnumSet;
use arrayvec::ArrayVec;

fn main() {
    let first_pc_bag = pcf::PIECES.iter()
        .chain(pcf::PIECES.iter())
        .chain(pcf::PIECES.iter())
        .copied().collect();
    let (send, recv) = crossbeam_channel::unbounded::<ArrayVec<[_; 10]>>();
    let t = std::time::Instant::now();
    pcf::find_combinations_mt(
        first_pc_bag, pcf::BitBoard(0), &AtomicBool::new(false), 4,
        move |combo| send.send(combo.iter().copied().collect()).unwrap()
    );
    let mut all_combinations = HashMap::<_, Vec<_>>::new();
    for combo in recv.into_iter() {
        let pieces: pcf::PieceSet = combo.iter().map(|p| p.kind.piece()).collect();
        all_combinations.entry(pieces).or_default().push(combo.into_inner().unwrap());
    }
    println!("Took {:?} to generate combinations", t.elapsed());


    let mut queued_bags = HashSet::new();
    let mut bags = vec![(BagWithHold::default(), 0)];
    queued_bags.insert(BagWithHold::default());
    let mut i = 0;
    let mut pcs = [vec![], vec![], vec![], vec![], vec![], vec![], vec![]];
    while let Some((initial_bag, pc_num)) = bags.pop() {
        i += 1;
        let skip = std::fs::metadata(&format!("pc-{}.ccbook", i)).is_ok();
        let mut book = BookBuilder::new();
        let (send, recv) = crossbeam_channel::bounded(256);
        let count = &std::sync::atomic::AtomicUsize::new(0);
        let t = std::time::Instant::now();
        pcs[pc_num].push(i);
        let mut all_seq = all_sequences(initial_bag);
        all_seq.retain(|(_,b)| b.hold.is_none() || b.bag == EnumSet::all());
        let total = all_seq.len();
        println!("Working on PC book {} ({} queues)", i, total);
        rayon::scope(|s| {
            for (seq, bag) in all_seq {
                if queued_bags.insert(bag) {
                    bags.push((bag, pc_num+1));
                }
                if skip { continue }
                let send = send.clone();
                let combos = all_combinations.get(
                    &seq.iter().copied().collect()
                ).map(|v| &**v).unwrap_or(&[]);
                s.spawn(move |_| {
                    for combo in combos {
                        pcf::solve_placement_combination(
                            &seq, pcf::BitBoard(0), combo, false, false, &AtomicBool::new(false),
                            pcf::placeability::simple_srs_spins,
                            |soln| send.send(process_soln(soln, initial_bag)).unwrap()
                        );
                    }
                    let c = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if 100*c / total != 100*(c+1) / total {
                        println!("{}%", 100*(c+1) / total);
                    }
                });
            }
            if skip { return }
            println!("Took {:?} to spawn solve tasks", t.elapsed());

            drop(send);

            let t = std::time::Instant::now();
            for soln in recv {
                let &(pos, mv) = soln.last().unwrap();
                book.add_move(pos, mv, Some(1.0));
                for &(pos, mv) in &soln {
                    book.add_move(pos, mv, None);
                }
            }
            println!("Took {:?} to add moves to the book", t.elapsed());
            println!("book is {} positions large", book.positions().count());
        });
        if skip { continue }

        let t = std::time::Instant::now();
        book.recalculate_graph();
        println!("Took {:?} to calculate the book", t.elapsed());
        let initial_position = libtetris::Board::new_with_state(
            [[false; 10]; 40], initial_bag.bag, initial_bag.hold, false, 0
        ).into();
        println!("{:?}", book.value_of_position(initial_position));

        let t = std::time::Instant::now();
        let f = std::fs::File::create(&format!("pc-{}.ccbook", i)).unwrap();
        book.compile(&[initial_position]).save(
            std::io::BufWriter::new(f)
        ).unwrap();
        println!("Took {:?} to save PC book {}", t.elapsed(), i);
    }

    for (i, book_set) in pcs.iter().enumerate() {
        println!("Merging books for PC {}", i);
        if std::fs::metadata(&format!("fullpc-{}.ccbook", i)).is_ok() {
            continue
        }
        let t = std::time::Instant::now();
        let mut iter = book_set.iter().map(|&n| {
            let f = std::fs::File::open(&format!("pc-{}.ccbook", n)).unwrap();
            Book::load(std::io::BufReader::new(f)).unwrap()
        });
        let mut book = iter.next().unwrap();
        for (i, b) in iter.enumerate() {
            book.merge(b);
            println!("{}%", i*100 / (book_set.len()-1));
        }
        println!("Saving book...");
        let f = std::fs::File::create(&format!("fullpc-{}.ccbook", i)).unwrap();
        book.save(std::io::BufWriter::new(f)).unwrap();
        println!("Took {:?}", t.elapsed());
    }

    println!("Merging books for complete PC book");
    let t = std::time::Instant::now();
    let mut iter = (0..7).map(|n| {
        let f = std::fs::File::open(&format!("fullpc-{}.ccbook", n)).unwrap();
        Book::load(std::io::BufReader::new(f)).unwrap()
    });
    let mut book = iter.next().unwrap();
    for (i, b) in iter.enumerate() {
        book.merge(b);
        println!("{}%", i*100 / 6);
    }
    println!("Saving book...");
    let f = std::fs::File::create("pc.ccbook").unwrap();
    book.save(std::io::BufWriter::new(f)).unwrap();
    println!("Took {:?}", t.elapsed());
}

fn process_soln(
    soln: &[pcf::Placement], bag: BagWithHold
) -> ArrayVec<[(Position, FallingPiece); 10]> {
    let mut poses = ArrayVec::new();
    let mut pos: Position = libtetris::Board::new_with_state(
        [[false; 10]; 40], bag.bag, bag.hold, false, 0
    ).into();
    let mut b = pcf::BitBoard(0);
    for p in soln {
        let mv = *p.srs_piece(b).first().unwrap();
        let mv = FallingPiece {
            kind: libtetris::PieceState(libtetris_piece(mv.piece), match mv.rotation {
                pcf::Rotation::West => libtetris::RotationState::West,
                pcf::Rotation::East => libtetris::RotationState::East,
                pcf::Rotation::North => libtetris::RotationState::North,
                pcf::Rotation::South => libtetris::RotationState::South,
            }),
            x: mv.x,
            y: mv.y,
            tspin: libtetris::TspinStatus::None
        };
        poses.push((pos, mv));
        b = b.combine(p.board());
        pos = pos.advance(mv).0;
    }
    poses
}

fn all_sequences(
    bag: BagWithHold
) -> Vec<([pcf::Piece; 10], BagWithHold)> {
    let mut result = vec![];
    all_sequences_impl(&mut result, &mut ArrayVec::new(), bag);
    result
}

fn all_sequences_impl(
    into: &mut Vec<([pcf::Piece; 10], BagWithHold)>,
    q: &mut ArrayVec<[pcf::Piece; 10]>,
    bag: BagWithHold
) {
    if q.is_full() {
        into.push((q.clone().into_inner().unwrap(), bag));
    } else {
        for p in bag {
            q.push(pcf_piece(p));
            all_sequences_impl(into, q, bag - p);
            q.pop();
        }
    }
}

fn libtetris_piece(p: pcf::Piece) -> libtetris::Piece {
    match p {
        pcf::Piece::I => libtetris::Piece::I,
        pcf::Piece::T => libtetris::Piece::T,
        pcf::Piece::O => libtetris::Piece::O,
        pcf::Piece::L => libtetris::Piece::L,
        pcf::Piece::J => libtetris::Piece::J,
        pcf::Piece::S => libtetris::Piece::S,
        pcf::Piece::Z => libtetris::Piece::Z,
    }
}

fn pcf_piece(p: libtetris::Piece) -> pcf::Piece {
    match p {
        libtetris::Piece::I => pcf::Piece::I,
        libtetris::Piece::T => pcf::Piece::T,
        libtetris::Piece::O => pcf::Piece::O,
        libtetris::Piece::L => pcf::Piece::L,
        libtetris::Piece::J => pcf::Piece::J,
        libtetris::Piece::S => pcf::Piece::S,
        libtetris::Piece::Z => pcf::Piece::Z,
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
struct BagWithHold {
    bag: EnumSet<libtetris::Piece>,
    hold: Option<libtetris::Piece>
}

impl Default for BagWithHold {
    fn default() -> Self {
        BagWithHold {
            bag: EnumSet::all(),
            hold: None
        }
    }
}

impl std::ops::Sub<libtetris::Piece> for BagWithHold {
    type Output = Self;
    fn sub(self, rhs: libtetris::Piece) -> Self {
        let base = match self.hold {
            Some(p) if p == rhs => BagWithHold {
                bag: self.bag,
                hold: None
            },
            _ => BagWithHold {
                bag: self.bag - rhs,
                hold: self.hold
            }
        };
        if base.bag.len() == 1 && base.hold.is_none() {
            BagWithHold {
                bag: EnumSet::all(),
                hold: base.bag.iter().next()
            }
        } else {
            base
        }
    }
}

impl IntoIterator for BagWithHold {
    type IntoIter = std::iter::Chain<enumset::EnumSetIter<libtetris::Piece>, std::option::IntoIter<libtetris::Piece>>;
    type Item = libtetris::Piece;
    fn into_iter(self) -> Self::IntoIter {
        self.bag.iter().chain(self.hold.into_iter())
    }
}
