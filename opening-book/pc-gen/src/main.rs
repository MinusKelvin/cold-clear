use opening_book::{ BookBuilder, Position };
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

    let mut book = BookBuilder::new();

    rayon::scope(|s| {
        let (send, recv) = crossbeam_channel::bounded(256);

        let mut queued_bags = HashSet::new();
        let mut bags = vec![EnumSet::empty()];
        queued_bags.insert(EnumSet::empty());
        let t = std::time::Instant::now();
        while let Some(initial_bag) = bags.pop() {
            for (seq, bag) in all_sequences(initial_bag) {
                if queued_bags.insert(bag) {
                    bags.push(bag);
                }
                let send = send.clone();
                let combos = all_combinations.get(
                    &seq.iter().copied().collect()
                ).map(|v| &**v).unwrap_or(&[]);
                s.spawn(move |_| for combo in combos {
                    pcf::solve_placement_combination(
                        &seq, pcf::BitBoard(0), combo, true, false, &AtomicBool::new(false),
                        pcf::placeability::simple_srs_spins,
                        |soln| send.send(process_soln(soln, initial_bag)).unwrap()
                    );
                });
            }
        }
        println!("Took {:?} to spawn solve tasks", t.elapsed());

        drop(send);

        let t = std::time::Instant::now();
        for soln in recv {
            for &(pos, mv) in &soln {
                book.add_move(pos, mv, None);
            }
            let &(pos, mv) = soln.last().unwrap();
            book.add_move(pos, mv, Some(1.0));
        }
        println!("Took {:?} to add moves to the book", t.elapsed());
    });

    let t = std::time::Instant::now();
    book.recalculate_graph();
    println!("Took {:?} to calculate the book", t.elapsed());

    let t = std::time::Instant::now();
    book.compile(&[libtetris::Board::new().into()]).save(
        std::fs::File::create("pc.ccbook").unwrap()
    ).unwrap();
    println!("Took {:?} to save the book", t.elapsed());
}

fn process_soln(
    soln: &[pcf::Placement], bag: EnumSet<libtetris::Piece>
) -> ArrayVec<[(Position, FallingPiece); 10]> {
    let mut poses = ArrayVec::new();
    let mut pos: Position = libtetris::Board::new_with_state(
        [[false; 10]; 40], bag, None, false, 0
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
    bag: EnumSet<libtetris::Piece>
) -> Vec<([pcf::Piece; 10], EnumSet<libtetris::Piece>)> {
    let mut result = vec![];
    all_sequences_impl(&mut result, &mut ArrayVec::new(), bag);
    result
}

fn all_sequences_impl(
    into: &mut Vec<([pcf::Piece; 10], EnumSet<libtetris::Piece>)>,
    q: &mut ArrayVec<[pcf::Piece; 10]>,
    mut bag: EnumSet<libtetris::Piece>
) {
    if q.is_full() {
        into.push((q.clone().into_inner().unwrap(), bag));
    } else {
        if bag.is_empty() {
            bag = EnumSet::all();
        }
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
