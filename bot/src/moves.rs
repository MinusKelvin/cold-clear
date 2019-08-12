use libtetris::{ Board, FallingPiece, Piece, RotationState, TspinStatus };
use arrayvec::ArrayVec;
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

type InputList = ArrayVec<[Input; 32]>;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Move {
    pub inputs: InputList,
    pub location: FallingPiece,
    pub soft_dropped: bool
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
    board: &Board,
    mut spawned: FallingPiece,
    mode: MovementMode
) -> Vec<Move> {
    let t = std::time::Instant::now();

    let mut locks = HashMap::with_capacity(1024);
    let mut checked = HashSet::with_capacity(1024);
    let mut check_queue = VecDeque::new();
    let fast_mode;

    if board.column_heights().iter().all(|&v| v < 16) {
        let starts = match mode {
            MovementMode::TwentyG => vec![(spawned, [Input::SonicDrop].iter().copied().collect())],
            MovementMode::ZeroG => zero_g_starts(spawned.kind.0),
            MovementMode::ZeroGFinesse => zero_g_finesse_starts(spawned.kind.0)
        };
        for (mut place, mut inputs) in starts {
            place.sonic_drop(board);
            checked.insert(place);
            lock_check(place, &mut locks, inputs.clone());
            inputs.push(Input::SonicDrop);
            check_queue.push_back((inputs, place));
        }
        fast_mode = mode != MovementMode::TwentyG;
    } else {
        fast_mode = false;
        let mut inputs = ArrayVec::new();
        if mode == MovementMode::TwentyG {
            spawned.sonic_drop(board);
            inputs.push(Input::SonicDrop);
        }
        checked.insert(spawned);
        check_queue.push_back((inputs, spawned));
    }

    while let Some((moves, position)) = check_queue.pop_front() {
        if !moves.is_full() {
            attempt(
                board, &moves, position,
                &mut checked, &mut check_queue,
                mode == MovementMode::TwentyG, fast_mode,
                Input::Left
            );
            attempt(
                board, &moves, position,
                &mut checked, &mut check_queue,
                mode == MovementMode::TwentyG, fast_mode,
                Input::Right
            );

            if position.kind.0 != Piece::O {
                attempt(
                    board, &moves, position,
                    &mut checked, &mut check_queue,
                    mode == MovementMode::TwentyG, fast_mode,
                    Input::Cw
                );

                attempt(
                    board, &moves, position,
                    &mut checked, &mut check_queue,
                    mode == MovementMode::TwentyG, fast_mode,
                    Input::Ccw
                );
            }

            if mode == MovementMode::ZeroGFinesse {
                attempt(
                    board, &moves, position,
                    &mut checked, &mut check_queue,
                    mode == MovementMode::TwentyG, fast_mode,
                    Input::DasLeft
                );

                attempt(
                    board, &moves, position,
                    &mut checked, &mut check_queue,
                    mode == MovementMode::TwentyG, fast_mode,
                    Input::DasRight
                );
            }
        }

        let mut change = position;
        if change.sonic_drop(board) && !moves.is_full() {
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::SonicDrop);
                check_queue.push_back((m, change));
            }
        }

        lock_check(change, &mut locks, moves);
    }

    let v: Vec<_> = locks.into_iter().map(|(_, v)| v).collect();
    unsafe {
        MOVES_FOUND += v.len();
        TIME_TAKEN += t.elapsed();
    }
    v
}

fn lock_check(
    piece: FallingPiece,
    locks: &mut HashMap<ArrayVec<[(i32, i32); 4]>, Move>,
    moves: InputList
) {
    let cells = piece.cells();
    if cells.iter().all(|&(_, y)| y >= 20) {
        return
    }
    match locks.entry(cells) {
        Entry::Vacant(entry) => {
            entry.insert(Move {
                soft_dropped: moves.contains(&Input::SonicDrop),
                inputs: moves,
                location: piece,
            });
        }
        Entry::Occupied(mut entry) => {
            let mv = entry.get_mut();
            if moves.len() < mv.inputs.len() {
                *mv = Move {
                    soft_dropped: moves.contains(&Input::SonicDrop),
                    inputs: moves,
                    location: piece,
                };
            }
        }
    }
}

fn attempt(
    board: &Board,
    moves: &InputList,
    mut piece: FallingPiece,
    checked: &mut HashSet<FallingPiece>,
    check_queue: &mut VecDeque<(InputList, FallingPiece)>,
    twenty_g: bool,
    fast_mode: bool,
    input: Input
) {
    if input.apply(&mut piece, board) {
        if !fast_mode || !board.above_stack(&piece) {
            let drop_input = twenty_g && piece.sonic_drop(board);
            if checked.insert(piece) {
                let mut m = moves.clone();
                m.push(input);
                if drop_input && !m.is_full() {
                    // If the move list is full this has to be the last movement anyways
                    // that is, it can't lead to positions unreachable under 20G.
                    m.push(Input::SonicDrop);
                }
                check_queue.push_back((m, piece));
            }
        }
    }
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

    fn apply(self, piece: &mut FallingPiece, board: &Board) -> bool {
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

fn zero_g_starts(p: Piece) -> Vec<(FallingPiece, InputList)> {
    use Piece::*;
    use RotationState::*;
    use Input::*;
    match p {
        O => vec![
            start(O, North, 4, &[]),
            start(O, North, 3, &[Left]),
            start(O, North, 5, &[Right]),
            start(O, North, 2, &[Left, Left]),
            start(O, North, 6, &[Right, Right]),
            start(O, North, 1, &[Left, Left, Left]),
            start(O, North, 7, &[Right, Right, Right]),
            start(O, North, 0, &[Left, Left, Left, Left]),
            start(O, North, 8, &[Right, Right, Right, Right]),
        ],
        I => vec![
            start(I, North, 4, &[]),
            start(I, North, 3, &[Left]),
            start(I, North, 5, &[Right]),
            start(I, North, 2, &[Left, Left]),
            start(I, North, 6, &[Right, Right]),
            start(I, North, 1, &[Left, Left, Left]),
            start(I, North, 7, &[Right, Right, Right]),
            start(I, West, 4, &[Ccw]),
            start(I, West, 3, &[Left, Ccw]),
            start(I, West, 2, &[Left, Ccw, Left]),
            start(I, West, 1, &[Left, Ccw, Left, Left]),
            start(I, West, 0, &[Left, Ccw, Left, Left, Left]),
            start(I, West, 5, &[Right, Ccw]),
            start(I, West, 6, &[Right, Ccw, Right]),
            start(I, West, 7, &[Right, Ccw, Right, Right]),
            start(I, West, 8, &[Right, Ccw, Right, Right, Right]),
            start(I, West, 9, &[Right, Ccw, Right, Right, Right, Right]),
            start(I, East, 4, &[Cw]),
            start(I, East, 3, &[Left, Cw]),
            start(I, East, 2, &[Left, Cw, Left]),
            start(I, East, 1, &[Left, Cw, Left, Left]),
            start(I, East, 0, &[Left, Cw, Left, Left, Left]),
            start(I, East, -1, &[Left, Cw, Left, Left, Left, Left]),
            start(I, East, 5, &[Right, Cw]),
            start(I, East, 6, &[Right, Cw, Right]),
            start(I, East, 7, &[Right, Cw, Right, Right]),
            start(I, East, 8, &[Right, Cw, Right, Right, Right]),
            start(I, South, 4, &[Cw, Cw]),
            start(I, South, 3, &[Cw, Left, Cw]),
            start(I, South, 5, &[Cw, Right, Cw]),
            start(I, South, 2, &[Cw, Left, Cw, Left]),
            start(I, South, 6, &[Cw, Right, Cw, Right]),
            start(I, South, 1, &[Left, Cw, Left, Cw, Left]),
            start(I, South, 7, &[Right, Cw, Right, Cw, Right]),
        ],
        _ => vec![
            start(p, North, 4, &[]),
            start(p, North, 3, &[Left]),
            start(p, North, 5, &[Right]),
            start(p, North, 2, &[Left, Left]),
            start(p, North, 6, &[Right, Right]),
            start(p, North, 1, &[Left, Left, Left]),
            start(p, North, 7, &[Right, Right, Right]),
            start(p, North, 8, &[Right, Right, Right, Right]),
            start(p, West, 4, &[Ccw]),
            start(p, West, 3, &[Left, Ccw]),
            start(p, West, 5, &[Right, Ccw]),
            start(p, West, 2, &[Left, Ccw, Left]),
            start(p, West, 6, &[Right, Ccw, Right]),
            start(p, West, 1, &[Left, Ccw, Left, Left]),
            start(p, West, 7, &[Right, Ccw, Right, Right]),
            start(p, West, 8, &[Right, Ccw, Right, Right, Right]),
            start(p, West, 9, &[Right, Ccw, Right, Right, Right, Right]),
            start(p, East, 4, &[Cw]),
            start(p, East, 3, &[Left, Cw]),
            start(p, East, 5, &[Right, Cw]),
            start(p, East, 2, &[Left, Cw, Left]),
            start(p, East, 6, &[Right, Cw, Right]),
            start(p, East, 1, &[Left, Cw, Left, Left]),
            start(p, East, 7, &[Right, Cw, Right, Right]),
            start(p, East, 0, &[Left, Cw, Left, Left, Left]),
            start(p, East, 8, &[Right, Cw, Right, Right, Right]),
            start(p, South, 4, &[Cw, Cw]),
            start(p, South, 3, &[Cw, Left, Cw]),
            start(p, South, 5, &[Cw, Right, Cw]),
            start(p, South, 2, &[Cw, Left, Cw, Left]),
            start(p, South, 6, &[Cw, Right, Cw, Right]),
            start(p, South, 1, &[Left, Cw, Left, Cw, Left]),
            start(p, South, 7, &[Right, Cw, Right, Cw, Right]),
            start(p, South, 8, &[Right, Cw, Right, Cw, Right, Right]),
        ]
    }
}

fn zero_g_finesse_starts(p: Piece) -> Vec<(FallingPiece, InputList)> {
    use Piece::*;
    use RotationState::*;
    use Input::*;
    match p {
        O => vec![
            start(O, North, 4, &[]),
            start(O, North, 3, &[Left]),
            start(O, North, 5, &[Right]),
            start(O, North, 2, &[Left, Left]),
            start(O, North, 6, &[Right, Right]),
            start(O, North, 1, &[DasLeft, Right]),
            start(O, North, 7, &[DasRight, Left]),
            start(O, North, 0, &[DasLeft]),
            start(O, North, 8, &[DasRight]),
        ],
        I => vec![
            start(I, North, 4, &[]),
            start(I, North, 3, &[Left]),
            start(I, North, 5, &[Right]),
            start(I, North, 2, &[Left, Left]),
            start(I, North, 6, &[Right, Right]),
            start(I, North, 1, &[DasLeft]),
            start(I, North, 7, &[DasRight]),
            start(I, West, 4, &[Ccw]),
            start(I, West, 3, &[Left, Ccw]),
            start(I, West, 2, &[Left, Ccw, Left]),
            start(I, West, 1, &[DasLeft, Ccw]),
            start(I, West, 0, &[Ccw, DasLeft]),
            start(I, West, 5, &[Right, Ccw]),
            start(I, West, 6, &[Right, Ccw, Right]),
            start(I, West, 7, &[DasRight, Ccw]),
            start(I, West, 8, &[DasRight, Ccw, Right]),
            start(I, West, 9, &[Ccw, DasRight]),
            start(I, East, 4, &[Cw]),
            start(I, East, 3, &[Left, Cw]),
            start(I, East, 2, &[Left, Cw, Left]),
            start(I, East, 1, &[DasLeft, Cw]),
            start(I, East, 0, &[Cw, DasLeft, Right]),
            start(I, East, -1, &[Cw, DasLeft]),
            start(I, East, 5, &[Right, Cw]),
            start(I, East, 6, &[Right, Cw, Right]),
            start(I, East, 7, &[DasRight, Cw]),
            start(I, East, 8, &[Cw, DasRight]),
            start(I, South, 4, &[Cw, Cw]),
            start(I, South, 3, &[Cw, Left, Cw]),
            start(I, South, 5, &[Cw, Right, Cw]),
            start(I, South, 2, &[Cw, Left, Cw, Left]),
            start(I, South, 6, &[Cw, Right, Cw, Right]),
            start(I, South, 1, &[Cw, Cw, DasLeft]),
            start(I, South, 7, &[Cw, Cw, DasRight]),
        ],
        _ => vec![
            start(p, North, 4, &[]),
            start(p, North, 3, &[Left]),
            start(p, North, 5, &[Right]),
            start(p, North, 2, &[Left, Left]),
            start(p, North, 6, &[Right, Right]),
            start(p, North, 1, &[DasLeft]),
            start(p, North, 7, &[DasRight, Left]),
            start(p, North, 8, &[DasRight]),
            start(p, West, 4, &[Ccw]),
            start(p, West, 3, &[Left, Ccw]),
            start(p, West, 5, &[Right, Ccw]),
            start(p, West, 2, &[Left, Ccw, Left]),
            start(p, West, 6, &[Right, Ccw, Right]),
            start(p, West, 1, &[DasLeft, Ccw]),
            start(p, West, 7, &[DasRight, Ccw, Left]),
            start(p, West, 8, &[DasRight, Ccw]),
            start(p, West, 9, &[Ccw, DasRight]),
            start(p, East, 4, &[Cw]),
            start(p, East, 3, &[Left, Cw]),
            start(p, East, 5, &[Right, Cw]),
            start(p, East, 2, &[Left, Cw, Left]),
            start(p, East, 6, &[Right, Cw, Right]),
            start(p, East, 1, &[DasLeft, Cw]),
            start(p, East, 7, &[DasRight, Cw, Left]),
            start(p, East, 0, &[Cw, DasLeft]),
            start(p, East, 8, &[DasRight, Cw]),
            start(p, South, 4, &[Cw, Cw]),
            start(p, South, 3, &[Cw, Left, Cw]),
            start(p, South, 5, &[Cw, Right, Cw]),
            start(p, South, 2, &[Cw, Left, Cw, Left]),
            start(p, South, 6, &[Cw, Right, Cw, Right]),
            start(p, South, 1, &[DasLeft, Cw, Cw]),
            start(p, South, 7, &[DasRight, Cw, Left, Cw]),
            start(p, South, 8, &[DasRight, Cw, Cw]),
        ]
    }
}

fn start(p: Piece, r: RotationState, x: i32, i: &[Input]) -> (FallingPiece, InputList) {
    (FallingPiece {
        kind: libtetris::PieceState(p, r),
        x,
        y: 19,
        tspin: TspinStatus::None
    }, i.iter().copied().collect())
}