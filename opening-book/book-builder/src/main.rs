use libtetris::*;
use std::io::prelude::*;
use opening_book::BookBuilder;

fn main() {
    let mut book = BookBuilder::new();

    for l in std::io::BufReader::new(std::io::stdin()).lines() {
        let fumen = match fumen::Fumen::decode(&l.unwrap()) {
            Ok(f) => f,
            Err(_) => continue
        };

        let mut field = [[false; 10]; 40];
        for y in 0..10 {
            for x in 0..10 {
                field[y][x] = fumen.pages[0].field[y][x] != fumen::CellColor::Empty;
            }
        }
        let mut comment_parts = fumen.pages[0].comment.as_deref().unwrap_or("").split('/');
        let bagspec = comment_parts.next().unwrap();
        let value = comment_parts.next().map(str::parse).map(Result::unwrap);

        let mut b = Board::new();
        b.set_field(field);
        b.bag = enumset::EnumSet::empty();
        for c in bagspec.chars() {
            let p = match c.to_ascii_uppercase() {
                'S' => Piece::S,
                'Z' => Piece::Z,
                'T' => Piece::T,
                'I' => Piece::I,
                'O' => Piece::O,
                'L' => Piece::L,
                'J' => Piece::J,
                _ => continue
            };
            if b.bag.contains(p) {
                b.hold_piece = Some(p);
            } else {
                b.bag |= p;
            }
        }
        if b.hold_piece.is_none() && b.bag.len() <= 1 {
            b.hold_piece = b.bag.iter().next();
            b.bag = enumset::EnumSet::all();
        }

        if fumen.pages.len() == 1 {
            let p = convert(fumen.pages[0].piece.unwrap());
            book.add_move(mirror_board(&b), mirror_placement(p), value);
            book.add_move(b, p, value);
        } else {
            let mut placements: Vec<_> = fumen.pages.iter().map(|p| {
                let mut b = Board::<u16>::new();
                let mut f = [[false; 10]; 40];
                for y in 0..10 {
                    for x in 0..10 {
                        f[y][x] = p.field[y][x] != fumen::CellColor::Empty;
                    }
                }
                b.set_field(f);
                let p = convert(p.piece.unwrap());
                (p, !b.above_stack(&p))
            }).collect();
            use permutator::Permutation;
            for permutation in placements.permutation() {
                let mut b = b.clone();
                let mut offset = 0;
                for (p, allow_sd) in permutation {
                    let p = FallingPiece {
                        y: p.y - offset,
                        ..p
                    };
                    if !b.on_stack(&p) || !allow_sd && !b.above_stack(&p) {
                        break
                    }
                    book.add_move(mirror_board(&b), mirror_placement(p), None);
                    book.add_move(&b, p, None);
                    b.add_next_piece(p.kind.0);
                    b.advance_queue();
                    offset += b.lock_piece(p).cleared_lines.len() as i32;
                }
            }
        }
    }

    let t = std::time::Instant::now();
    book.recalculate_graph();
    println!("{:?}", t.elapsed());

    dbg!(book.value_of_position(Board::new().into()));

    book.compile(&[Board::new().into()]).save(
        std::fs::File::create("book.ccbook").unwrap()
    ).unwrap();

    dump(&book);
}

fn convert(p: fumen::Piece) -> FallingPiece {
    FallingPiece {
        kind: PieceState(match p.kind {
            fumen::PieceType::I => Piece::I,
            fumen::PieceType::O => Piece::O,
            fumen::PieceType::T => Piece::T,
            fumen::PieceType::L => Piece::L,
            fumen::PieceType::J => Piece::J,
            fumen::PieceType::S => Piece::S,
            fumen::PieceType::Z => Piece::Z
        }, match p.rotation {
            fumen::RotationState::East => RotationState::East,
            fumen::RotationState::West => RotationState::West,
            fumen::RotationState::South => RotationState::South,
            fumen::RotationState::North => RotationState::North,
        }),
        x: p.x as i32,
        y: p.y as i32,
        tspin: TspinStatus::None
    }
}

fn mirror_board(b: &Board) -> Board {
    let mut b = b.clone();
    b.bag = b.bag.iter().map(mirror_piece).collect();
    b.hold_piece = b.hold_piece.map(mirror_piece);
    let mut f = b.get_field();
    for r in &mut f[..] {
        r.reverse();
    }
    b.set_field(f);
    b
}

fn mirror_piece(p: Piece) -> Piece {
    match p {
        Piece::J => Piece::L,
        Piece::L => Piece::J,
        Piece::S => Piece::Z,
        Piece::Z => Piece::S,
        _ => p
    }
}

fn mirror_placement(p: FallingPiece) -> FallingPiece {
    FallingPiece {
        kind: PieceState(mirror_piece(p.kind.0), match p.kind.1 {
            RotationState::East => RotationState::West,
            RotationState::West => RotationState::East,
            r => r
        }),
        x: match p.kind {
            PieceState(Piece::I, RotationState::North) => 8 - p.x,
            PieceState(Piece::I, RotationState::South) => 10 - p.x,
            PieceState(Piece::O, RotationState::North) => 8 - p.x,
            PieceState(Piece::O, RotationState::South) => 10 - p.x,
            _ => 9 - p.x
        },
        y: match p.kind {
            PieceState(Piece::I, RotationState::West) => p.y + 1,
            PieceState(Piece::I, RotationState::East) => p.y - 1,
            PieceState(Piece::O, RotationState::West) => p.y + 1,
            PieceState(Piece::O, RotationState::East) => p.y - 1,
            _ => p.y
        },
        tspin: p.tspin
    }
}

fn dump(book: &opening_book::BookBuilder) {
    fn name(pos: opening_book::Position) -> String {
        let mut s = String::new();
        for &r in pos.rows() {
            s.push_str(&format!("{},", r));
        }
        for p in pos.bag() {
            s.push(p.to_char());
        }
        if let Some(p) = pos.extra() {
            s.push(p.to_char());
        }
        s
    }
    std::fs::create_dir_all("book").unwrap();
    for pos in book.positions() {
        let mut f = std::fs::File::create(format!("book/{}.html", name(pos))).unwrap();
        write!(f, r"
            <DOCTYPE html>
            <html>
            <head>
                <style>
                    td {{
                        width: 16px;
                        border: 1px solid black;
                    }}
                    td::after {{
                        content: '';
                        margin-top: 100%;
                        display: block;
                    }}
                    table {{
                        border-collapse: collapse;
                    }}
                    a {{
                        display: inline-block;
                    }}
                </style>
            </head>
            <body>"
        ).unwrap();
        let value = book.value_of_position(pos);
        write!(f, "<p>E(V): {:.5}", value.value).unwrap();
        write!(f, "<br>E(t): {:.5}", value.long_moves).unwrap();
        write!(f, "<p>Bag: ").unwrap();
        for p in pos.bag().iter().chain(pos.extra().iter().copied()) {
            write!(f, "{}", p.to_char()).unwrap();
        }
        write!(f, "<p>").unwrap();
        let mut moves: Vec<_> = book.moves(pos).into_iter()
            .map(|mv| (mv, book.value_of_position(pos.advance(mv.location).0)))
            .collect();
        moves.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().reverse());
        for (mv, v) in moves {
            let cells = mv.location.cells();
            write!(f, "<a href='{}.html'>", name(pos.advance(mv.location).0)).unwrap();
            match mv.value {
                Some(v) => write!(f, "V={}", v).unwrap(),
                None => {
                    write!(f, "E(V)={:.5}", v.value).unwrap();
                    write!(f, "<br>E(t)={:.5}", v.long_moves).unwrap();
                }
            };
            write!(f, "<table>").unwrap();
            for y in (0..10).rev() {
                write!(f, "<tr>").unwrap();
                for x in 0..10 {
                    write!(
                        f, "<td style='background-color: {}'></td>",
                        if pos.rows()[y] & 1<<x != 0 {
                            "gray"
                        } else if cells.contains(&(x, y as i32)) {
                            match mv.location.kind.0 {
                                Piece::I => "cyan",
                                Piece::J => "blue",
                                Piece::L => "orange",
                                Piece::Z => "red",
                                Piece::S => "green",
                                Piece::T => "purple",
                                Piece::O => "yellow"
                            }
                        } else {
                            "black"
                        }
                    ).unwrap();
                }
                write!(f, "</tr>").unwrap();
            }
            write!(f, "</table></a> ").unwrap();
        }
        if pos.bag() == enumset::EnumSet::all() {
            for (next, b) in pos.next_possibilities() {
                for (queue, bag) in opening_book::possible_sequences(vec![], b) {
                    let v = book.value_of_raw(pos, next, &queue, bag);
                    if v == Default::default() {
                        write!(f, "<p>({:?}){:?} = {:?}", next, queue, v).unwrap();
                    }
                }
            }
        }
        write!(f, "</body></html>").unwrap();
    }
}