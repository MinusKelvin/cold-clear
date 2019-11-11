use lazy_static::lazy_static;
use std::collections::HashMap;
use enumset::EnumSet;
use libtetris::{ Board, Piece, FallingPiece };

mod viper;
mod pco;

type OpenerBook = HashMap<([u16; 10], EnumSet<Piece>), (Opener, i32)>;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Opener {
    Viper,
    Pco
}

pub fn get(board: &Board) -> Option<(Opener, i32)> {
    let field = field(board);
    let mut bag = board.get_bag();
    for piece in board.next_queue().rev() {
        if bag == EnumSet::all() {
            bag = EnumSet::empty();
        }
        bag.insert(piece);
    }
    if let Some(piece) = board.hold_piece() {
        if bag == EnumSet::all() {
            let v = check(field, EnumSet::only(piece));
            if v.is_some() {
                return v;
            }
        }
        bag.insert(piece);
    }

    check(field, bag)
}

fn check(field: [u16; 10], bag: EnumSet<Piece>) -> Option<(Opener, i32)> {
    let v = OPENER_BOOK.get(&(field, bag)).cloned();
    if v.is_some() {
        return v;
    }
    let mut mirrored = [0; 10];
    for y in 0..10 {
        let mut v = 0;
        for x in 0..10 {
            v <<= 1;
            v |= (field[y] & 1<<x != 0) as u16;
        }
        mirrored[y] = v;
    }
    let mut b = EnumSet::empty();
    for piece in bag.iter() {
        b |= match piece {
            Piece::L => Piece::J,
            Piece::J => Piece::L,
            Piece::S => Piece::Z,
            Piece::Z => Piece::S,
            _ => piece
        };
    }
    OPENER_BOOK.get(&(mirrored, b)).cloned()
}

lazy_static! {
    static ref OPENER_BOOK: OpenerBook = {
        let mut states = HashMap::new();

        viper::init(&mut states);
        pco::init(&mut states);

        for ((board, bag), opener) in states.iter() {
            print!("{:?}, ", opener);
            for piece in bag.iter() {
                print!("{}", piece.to_char());
            }
            println!();
            for row in board.iter().rev() {
                for x in 0..10 {
                    if row & 1<<x != 0 {
                        print!("[]");
                    } else {
                        print!("..");
                    }
                }
                println!()
            }
        }

        states
    };
}

fn board(low_field: [u16; 10]) -> Board {
    let mut field = [[false; 10]; 40];
    for y in 0..10 {
        for x in 0..10 {
            if low_field[y] & 1<<x != 0 {
                field[y][x] = true;
            }
        }
    }
    let mut b = Board::new();
    b.set_field(field);
    b
}

fn field(board: &Board) -> [u16; 10] {
    let mut field = [0; 10];
    for y in 0..10 {
        field[y] = *board.get_row(y as i32);
    }
    field
}

fn build_states(
    states: &mut OpenerBook,
    placements: &[FallingPiece],
    from_board: [u16; 10],
    from_bag: EnumSet<Piece>,
    opener: Opener,
    encouragement: i32
) {
    for &placement in placements {
        let mut board = board(from_board);
        let possibilities = crate::moves::find_moves(&board, FallingPiece {
            y: 20, ..placement
        }, crate::moves::MovementMode::ZeroG);

        let mut placeable = false;
        let target = placement.cells();
        for mv in possibilities {
            let result = mv.location.cells();
            for pos in result {
                if target.contains(&pos) {
                    placeable = true;
                    continue;
                }
            }
        }

        if placeable {
            let result = board.lock_piece(placement);
            let mut bag = from_bag - placement.kind.0;
            if bag.is_empty() {
                bag = EnumSet::all();
            }
            let field = field(&board);
            let bonus = if placements.len() == 1 { encouragement } else { 0 };
            states.insert((field, bag), (opener, bonus));

            let mut new_placements = Vec::with_capacity(placements.len() - 1);
            for &p in placements {
                if p != placement {
                    let fall_amount = result.cleared_lines.iter().filter(|&&y| y <= p.y).count();
                    new_placements.push(FallingPiece {
                        y: p.y - fall_amount as i32, ..p
                    })
                }
            }
            build_states(states, &new_placements, field, bag, opener, encouragement);
        }
    }
}