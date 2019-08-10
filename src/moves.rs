use crate::tetris::{ BoardState, FallingPiece };
use std::collections::{ HashMap, HashSet, VecDeque, hash_map::Entry };

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Input {
    Left,
    Right,
    Cw,
    Ccw,
    SonicDrop,
    DasLeft,
    DasRight
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Move {
    pub inputs: Vec<Input>,
    pub location: FallingPiece
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MovementMode {
    ZeroG,
    ZeroGFinesse,
    TwentyG,
}

pub static mut TIME_TAKEN: std::time::Duration = std::time::Duration::from_secs(0);
pub static mut MOVES_FOUND: usize = 0;

pub fn find_moves(
    board: &BoardState,
    mut spawned: FallingPiece,
    mode: MovementMode
) -> Vec<Move> {
    let t = std::time::Instant::now();

    let mut locks = HashMap::new();
    let mut checked = HashSet::new();
    let mut check_queue = VecDeque::new();

    if mode == MovementMode::TwentyG {
        spawned.sonic_drop(board);
        check_queue.push_back((vec![Input::SonicDrop], spawned));
    } else {
        check_queue.push_back((vec![], spawned));
    }

    while let Some((moves, position)) = check_queue.pop_front() {
        let mut change = position;
        if change.shift(board, -1) {
            let drop_input = mode == MovementMode::TwentyG && change.sonic_drop(board);
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::Left);
                if drop_input {
                    m.push(Input::SonicDrop);
                }
                check_queue.push_back((m, change));
            }
        }

        let mut change = position;
        if change.shift(board, 1) {
            let drop_input = mode == MovementMode::TwentyG && change.sonic_drop(board);
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::Right);
                if drop_input {
                    m.push(Input::SonicDrop);
                }
                check_queue.push_back((m, change));
            }
        }

        let mut change = position;
        if change.cw(board) {
            let drop_input = mode == MovementMode::TwentyG && change.sonic_drop(board);
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::Cw);
                if drop_input {
                    m.push(Input::SonicDrop);
                }
                check_queue.push_back((m, change));
            }
        }

        let mut change = position;
        if change.ccw(board) {
            let drop_input = mode == MovementMode::TwentyG && change.sonic_drop(board);
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::Ccw);
                if drop_input {
                    m.push(Input::SonicDrop);
                }
                check_queue.push_back((m, change));
            }
        }

        if mode == MovementMode::ZeroGFinesse {
            let mut change = position;
            while change.shift(board, -1) {}
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::DasLeft);
                check_queue.push_back((m, change));
            }

            let mut change = position;
            while change.shift(board, 1) {}
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::DasRight);
                check_queue.push_back((m, change));
            }
        }

        let mut change = position;
        if change.sonic_drop(board) {
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::SonicDrop);
                check_queue.push_back((m, change));
            }
        }

        let cells = change.cells();
        if cells.iter().all(|&(_, y)| y >= 20) {
            continue
        }
        match locks.entry(cells) {
            Entry::Vacant(entry) => {
                entry.insert(Move {
                    inputs: moves,
                    location: change
                });
            }
            Entry::Occupied(mut entry) => {
                let mv = entry.get_mut();
                let us_sdrops = moves.iter().filter(|&&v| v == Input::SonicDrop).count();
                let them_sdrops = mv.inputs.iter().filter(|&&v| v == Input::SonicDrop).count();
                if us_sdrops < them_sdrops ||
                        (us_sdrops == them_sdrops && moves.len() < mv.inputs.len()) {
                    *mv = Move {
                        inputs: moves,
                        location: change
                    };
                }
            }
        }
    }

    let v: Vec<_> = locks.into_iter().map(|(_, v)| v).collect();
    unsafe {
        TIME_TAKEN += t.elapsed();
        MOVES_FOUND += v.len();
    }
    v
}

impl Input {
}

impl Input {
    pub fn to_char(self) -> char {
        match self {
            Input::Left => '<',
            Input::Right => '>',
            Input::Cw => 'r',
            Input::Ccw => 'l',
            Input::SonicDrop => 'v',
            Input::DasLeft => '[',
            Input::DasRight => ']'
        }
    }

    fn apply(self, piece: &mut FallingPiece, board: &BoardState) -> bool {
        match self {
            Input::Left => piece.shift(board, -1),
            Input::Right => piece.shift(board, 1),
            Input::Ccw => piece.ccw(board),
            Input::Cw => piece.cw(board),
            Input::DasLeft => {
                let mut did = false;
                while piece.shift(board, -1) {
                    did = true;
                }
                did
            }
            Input::DasRight => {
                let mut did = false;
                while piece.shift(board, 1) {
                    did = true;
                }
                did
            }
            Input::SonicDrop => piece.sonic_drop(board)
        }
    }
}

// #[test]
// fn not_a_tspin_test() {
//     let mut board = BoardState::new();
//     let mut displays = vec![];

//     board.cells[6] = [false, false, true,  true,  true,  true,  true,  true,  false, false];
//     board.cells[5] = [false, false, false, true,  true,  true,  true,  false, false, false];
//     board.cells[4] = [false, true,  true,  false, true,  true,  false, false, false, false];
//     board.cells[3] = [true,  true,  false, true,  true,  false, false, false, false, false];
//     board.cells[2] = [true,  true,  true,  false, true,  false, true,  true,  true,  true];
//     board.cells[1] = [false, false, true,  false, true,  false, false, false, false, true];
//     board.cells[0] = [true,  true,  true,  true,  true,  false, false, false, true,  true];

//     let mut mv = Move {
//         inputs: vec![],
//         location: FallingPiece::spawn(crate::tetris::Piece::T, &board).unwrap()
//     };

//     for m in vec![
//         Input::SonicDrop,
//         Input::Right,
//         Input::Right,
//         Input::Right,
//         Input::Right,
//         Input::Cw,
//         Input::SonicDrop,
//         Input::Left,
//         Input::Ccw,
//         Input::SonicDrop,
//         Input::Left,
//         Input::Cw,
//         Input::Ccw
//     ] {
//         m.apply(&mut mv.location, &board);
//         mv.inputs.push(m);
//         let mut disp = crate::display::draw_move(&board, &mv, 0, Default::default());
//         disp[25].clear();
//         disp[25].push_str(match mv.location.tspin {
//             crate::tetris::TspinStatus::None           => "none        ",
//             crate::tetris::TspinStatus::Mini           => "mini t-spin ",
//             crate::tetris::TspinStatus::Full           => "t-spin      ",
//             crate::tetris::TspinStatus::PersistentFull => "tst t-spin  ",
//         });
//         displays.push(disp);
//     }

//     crate::display::write_drawings(&mut std::io::stdout(), &displays).unwrap();
// }