use libtetris::*;
use std::io::prelude::*;

fn main() {
    let mut book = opening_book::Book::new();

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
        let mut comment_parts = fumen.pages[0].comment.as_ref().unwrap().split('/');
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

    book.dump();
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
            PieceState(Piece::I, RotationState::South) => 8 - p.x,
            PieceState(Piece::O, RotationState::North) => 8 - p.x,
            PieceState(Piece::O, RotationState::South) => 8 - p.x,
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