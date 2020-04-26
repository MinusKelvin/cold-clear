use libtetris::{ Board, FallingPiece, Piece, RotationState, TspinStatus, PieceMovement };
use arrayvec::ArrayVec;
use std::collections::{ HashMap, HashSet, hash_map::Entry };
use serde::{ Serialize, Deserialize };

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct InputList {
    pub movements: ArrayVec<[PieceMovement; 32]>,
    pub time: u32
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub inputs: InputList,
    pub location: FallingPiece
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Move {
    pub inputs: ArrayVec<[PieceMovement; 32]>,
    pub expected_location: FallingPiece,
    pub hold: bool
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum MovementMode {
    ZeroG,
    ZeroGComplete,
    TwentyG,
    HardDropOnly
}

pub fn find_moves(
    board: &Board,
    mut spawned: FallingPiece,
    mode: MovementMode
) -> Vec<Placement> {
    let mut locks = HashMap::with_capacity(1024);
    let mut checked = HashSet::with_capacity(1024);
    let mut check_queue = vec![];
    let fast_mode;

    if board.column_heights().iter().all(|&v| v < 16) {
        // We know that we can reach any column and rotation state without bumping into the terrain
        // at 0G here, so we can just grab those starting positions.
        let starts = match mode {
            MovementMode::TwentyG => vec![
                (spawned, InputList {
                    movements: ArrayVec::new(),
                    time: 0
                })
            ],
            _ => zero_g_starts(spawned.kind.0),
        };
        // Fast mode prevents checking a lot of stack movement that is unlikely (but still could)
        // to lead to new placements. Use ZeroGComplete to get these missed positions.
        fast_mode = mode == MovementMode::ZeroG;
        for (mut place, mut inputs) in starts {
            let orig_y = place.y;
            place.sonic_drop(board);
            if !fast_mode {
                checked.insert(place);
            }
            lock_check(place, &mut locks, inputs.clone());
            if mode != MovementMode::HardDropOnly {
                // Initialize stack movement starting positions.
                inputs.movements.push(PieceMovement::SonicDrop);
                if mode != MovementMode::TwentyG {
                    inputs.time += 2 * (orig_y - place.y) as u32;
                }
                check_queue.push(Placement { inputs, location: place });
            }
        }
    } else {
        fast_mode = false;
        let mut movements = ArrayVec::new();
        if mode == MovementMode::TwentyG {
            spawned.sonic_drop(board);
            movements.push(PieceMovement::SonicDrop);
        }
        checked.insert(spawned);
        check_queue.push(Placement {
            inputs: InputList { movements, time: 0 },
            location: spawned
        });
    }

    fn next(q: &mut Vec<Placement>) -> Option<Placement> {
        q.sort_by_key(|p| std::u32::MAX-p.inputs.time);
        q.pop()
    }

    while let Some(placement) = next(&mut check_queue) {
        let moves = placement.inputs;
        let position = placement.location;
        if !moves.movements.is_full() {
            attempt(
                board, &moves, position,
                &mut checked, &mut check_queue,
                mode, fast_mode,
                PieceMovement::Left, false
            );
            attempt(
                board, &moves, position,
                &mut checked, &mut check_queue,
                mode, fast_mode,
                PieceMovement::Right, false
            );

            if position.kind.0 != Piece::O {
                attempt(
                    board, &moves, position,
                    &mut checked, &mut check_queue,
                    mode, fast_mode,
                    PieceMovement::Cw, false
                );

                attempt(
                    board, &moves, position,
                    &mut checked, &mut check_queue,
                    mode, fast_mode,
                    PieceMovement::Ccw, false
                );
            }

            if mode == MovementMode::ZeroG {
                attempt(
                    board, &moves, position,
                    &mut checked, &mut check_queue,
                    mode, fast_mode,
                    PieceMovement::Left, true
                );

                attempt(
                    board, &moves, position,
                    &mut checked, &mut check_queue,
                    mode, fast_mode,
                    PieceMovement::Right, true
                );
            }

            attempt(
                board, &moves, position,
                &mut checked, &mut check_queue,
                mode, fast_mode,
                PieceMovement::SonicDrop, false
            );
        }

        let mut position = position;
        position.sonic_drop(board);
        lock_check(position, &mut locks, moves);
    }

    locks.into_iter().map(|(_, v)| v).collect()
}

fn lock_check(
    piece: FallingPiece,
    locks: &mut HashMap<(ArrayVec<[(i32, i32); 4]>, TspinStatus), Placement>,
    moves: InputList
) {
    let cells = piece.cells();
    if cells.iter().all(|&(_, y, _)| y >= 20) {
        return
    }
    match locks.entry((cells.iter().map(|&(x,y,_)|(x,y)).collect(), piece.tspin)) {
        Entry::Vacant(entry) => {
            entry.insert(Placement {
                inputs: moves,
                location: piece,
            });
        }
        Entry::Occupied(mut entry) => {
            let mv = entry.get_mut();
            if moves.time < mv.inputs.time {
                *mv = Placement {
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
    check_queue: &mut Vec<Placement>,
    mode: MovementMode,
    fast_mode: bool,
    input: PieceMovement,
    repeat: bool
) -> FallingPiece {
    let orig_y = piece.y;
    if input.apply(&mut piece, board) {
        let mut moves = moves.clone();
        if input == PieceMovement::SonicDrop {
            // We don't actually know the soft drop speed, but 1 cell every 2 ticks is probably a
            // decent guess - that's what the battle library's default game configuration has, and
            // it's also pretty close to Puyo Puyo Tetris's versus mode.
            moves.time += 2 * (orig_y - piece.y) as u32;
        } else {
            moves.time += 1;
        }
        if let Some(&m) = moves.movements.last() {
            if m == input {
                // Delay from releasing button before pressing it again
                moves.time += 1;
            }
        }
        moves.movements.push(input);
        while repeat && !moves.movements.is_full() && input.apply(&mut piece, board) {
            // This is the DAS left/right case
            moves.movements.push(input);
            moves.time += 2;
        }
        if !fast_mode || piece.tspin != TspinStatus::None || !board.above_stack(&piece) {
            // 20G causes instant plummet, but we might actually be playing a high gravity mode
            // that we're approximating as 20G so we need to add a sonic drop movement to signal to
            // the input engine that we need the piece to hit the ground before continuing.
            let drop_input = mode == MovementMode::TwentyG && piece.sonic_drop(board);
            if checked.insert(piece) {
                if drop_input && !moves.movements.is_full() {
                    // We need the sonic drop input for the above reason, but if the move list is
                    // full this has to be the last move and the input engine should hard drop.
                    moves.movements.push(PieceMovement::SonicDrop);
                }
                if !(mode == MovementMode::HardDropOnly && input == PieceMovement::SonicDrop) {
                    check_queue.push(Placement { inputs: moves, location: piece });
                }
            }
        }
    }
    piece
}

fn zero_g_starts(p: Piece) -> Vec<(FallingPiece, InputList)> {
    use Piece::*;
    use RotationState::*;
    use PieceMovement::*;
    match p {
        O => vec![
            start(O, North, 4, &[], 0),
            start(O, North, 3, &[Left], 1),
            start(O, North, 5, &[Right], 1),
            start(O, North, 2, &[Left, Left], 3),
            start(O, North, 6, &[Right, Right], 3),
            start(O, North, 1, &[Left, Left, Left], 5),
            start(O, North, 7, &[Right, Right, Right], 5),
            start(O, North, 0, &[Left, Left, Left, Left], 7),
            start(O, North, 8, &[Right, Right, Right, Right], 7),
        ],
        I => vec![
            start(I, North, 4, &[], 0),
            start(I, North, 3, &[Left], 1),
            start(I, North, 5, &[Right], 1),
            start(I, North, 2, &[Left, Left], 3),
            start(I, North, 6, &[Right, Right], 3),
            start(I, North, 1, &[Left, Left, Left], 5),
            start(I, North, 7, &[Right, Right, Right], 5),
            start(I, West, 4, &[Ccw], 1),
            start(I, West, 3, &[Left, Ccw], 2),
            start(I, West, 2, &[Left, Ccw, Left], 3),
            start(I, West, 1, &[Left, Ccw, Left, Left], 5),
            start(I, West, 0, &[Left, Ccw, Left, Left, Left], 7),
            start(I, West, 5, &[Right, Ccw], 2),
            start(I, West, 6, &[Right, Ccw, Right], 3),
            start(I, West, 7, &[Right, Ccw, Right, Right], 5),
            start(I, West, 8, &[Right, Ccw, Right, Right, Right], 7),
            start(I, West, 9, &[Right, Ccw, Right, Right, Right, Right], 9),
            start(I, East, 5, &[Cw], 1),
            start(I, East, 4, &[Left, Cw], 2),
            start(I, East, 3, &[Left, Cw, Left], 3),
            start(I, East, 2, &[Left, Cw, Left, Left], 5),
            start(I, East, 1, &[Left, Cw, Left, Left, Left], 7),
            start(I, East, 0, &[Left, Cw, Left, Left, Left, Left], 9),
            start(I, East, 6, &[Right, Cw], 2),
            start(I, East, 7, &[Right, Cw, Right], 3),
            start(I, East, 8, &[Right, Cw, Right, Right], 5),
            start(I, East, 9, &[Right, Cw, Right, Right, Right], 7),
            start(I, South, 5, &[Cw, Cw], 3),
            start(I, South, 4, &[Cw, Left, Cw], 3),
            start(I, South, 6, &[Cw, Right, Cw], 3),
            start(I, South, 3, &[Cw, Left, Cw, Left], 4),
            start(I, South, 7, &[Cw, Right, Cw, Right], 4),
            start(I, South, 2, &[Left, Cw, Left, Cw, Left], 5),
            start(I, South, 8, &[Right, Cw, Right, Cw, Right], 5),
        ],
        _ => vec![
            start(p, North, 4, &[], 0),
            start(p, North, 3, &[Left], 1),
            start(p, North, 5, &[Right], 1),
            start(p, North, 2, &[Left, Left], 3),
            start(p, North, 6, &[Right, Right], 3),
            start(p, North, 1, &[Left, Left, Left], 5),
            start(p, North, 7, &[Right, Right, Right], 5),
            start(p, North, 8, &[Right, Right, Right, Right], 7),
            start(p, West, 4, &[Ccw], 1),
            start(p, West, 3, &[Left, Ccw], 2),
            start(p, West, 5, &[Right, Ccw], 2),
            start(p, West, 2, &[Left, Ccw, Left], 3),
            start(p, West, 6, &[Right, Ccw, Right], 3),
            start(p, West, 1, &[Left, Ccw, Left, Left], 5),
            start(p, West, 7, &[Right, Ccw, Right, Right], 5),
            start(p, West, 8, &[Right, Ccw, Right, Right, Right], 7),
            start(p, West, 9, &[Right, Ccw, Right, Right, Right, Right], 9),
            start(p, East, 4, &[Cw], 1),
            start(p, East, 3, &[Left, Cw], 2),
            start(p, East, 5, &[Right, Cw], 2),
            start(p, East, 2, &[Left, Cw, Left], 3),
            start(p, East, 6, &[Right, Cw, Right], 3),
            start(p, East, 1, &[Left, Cw, Left, Left], 5),
            start(p, East, 7, &[Right, Cw, Right, Right], 5),
            start(p, East, 0, &[Left, Cw, Left, Left, Left], 7),
            start(p, East, 8, &[Right, Cw, Right, Right, Right], 7),
            start(p, South, 4, &[Cw, Cw], 3),
            start(p, South, 3, &[Cw, Left, Cw], 3),
            start(p, South, 5, &[Cw, Right, Cw], 3),
            start(p, South, 2, &[Cw, Left, Cw, Left], 4),
            start(p, South, 6, &[Cw, Right, Cw, Right], 4),
            start(p, South, 1, &[Left, Cw, Left, Cw, Left], 5),
            start(p, South, 7, &[Right, Cw, Right, Cw, Right], 5),
            start(p, South, 8, &[Right, Cw, Right, Cw, Right, Right], 7),
        ]
    }
}

fn start(
    p: Piece, r: RotationState, x: i32, i: &[PieceMovement], time: u32
) -> (FallingPiece, InputList) {
    (FallingPiece {
        kind: libtetris::PieceState(p, r),
        x,
        y: 19,
        tspin: TspinStatus::None
    }, InputList {
        movements: i.iter().copied().collect(),
        time
    })
}