use arrayvec::ArrayVec;
use std::collections::VecDeque;
use libtetris::{ Board, LockResult, PlacementKind };

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Evaluation {
    pub accumulated: i32,
    pub transient: i32
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Default)]
pub struct BoardWeights {
    pub back_to_back: i32,
    pub bumpiness: i32,
    pub bumpiness_sq: i32,
    pub height: i32,
    pub top_half: i32,
    pub top_quarter: i32,
    pub cavity_cells: i32,
    pub cavity_cells_sq: i32,
    pub overhang_cells: i32,
    pub overhang_cells_sq: i32,
    pub covered_cells: i32,
    pub covered_cells_sq: i32,
    pub tslot_present: i32,
    pub well_depth: i32,
    pub max_well_depth: i32
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Default)]
pub struct PlacementWeights {
    pub b2b_clear: i32,
    pub clear1: i32,
    pub clear2: i32,
    pub clear3: i32,
    pub clear4: i32,
    pub tspin1: i32,
    pub tspin2: i32,
    pub tspin3: i32,
    pub mini_tspin1: i32,
    pub mini_tspin2: i32,
    pub perfect_clear: i32,
    pub combo_table: [i32; 12],
    pub soft_drop: i32
}

pub static mut TIME_TAKEN: std::time::Duration = std::time::Duration::from_secs(0);
pub static mut BOARDS_EVALUATED: usize = 0;

pub fn evaluate(
    lock: &LockResult,
    board: &Board,
    board_weights: &BoardWeights,
    placement_weights: &PlacementWeights,
    soft_dropped: bool
) -> Evaluation {
    let t = std::time::Instant::now();

    let mut transient_eval = 0;
    let mut acc_eval = 0;

    if lock.perfect_clear {
        acc_eval += placement_weights.perfect_clear;
    } else {
        if lock.b2b {
            acc_eval += placement_weights.b2b_clear;
        }
        if let Some(combo) = lock.combo {
            let combo = combo.min(11) as usize;
            acc_eval += placement_weights.combo_table[combo];
        }
        match lock.placement_kind {
            PlacementKind::Clear1 => {
                acc_eval += placement_weights.clear1;
            }
            PlacementKind::Clear2 => {
                acc_eval += placement_weights.clear2;
            }
            PlacementKind::Clear3 => {
                acc_eval += placement_weights.clear3;
            }
            PlacementKind::Clear4 => {
                acc_eval += placement_weights.clear4;
            }
            PlacementKind::Tspin1 => {
                acc_eval += placement_weights.tspin1;
            }
            PlacementKind::Tspin2 => {
                acc_eval += placement_weights.tspin2;
            }
            PlacementKind::Tspin3 => {
                acc_eval += placement_weights.tspin3;
            }
            PlacementKind::MiniTspin1 => {
                acc_eval += placement_weights.mini_tspin1;
            }
            PlacementKind::MiniTspin2 => {
                acc_eval += placement_weights.mini_tspin2;
            }
            _ => {}
        }
    }

    if soft_dropped {
        acc_eval += placement_weights.soft_drop;
    }

    if board.has_back_to_back_active() {
        transient_eval += board_weights.back_to_back;
    }

    let highest_point = *board.column_heights().iter().max().unwrap() as i32;
    transient_eval += board_weights.height * highest_point;
    transient_eval += board_weights.top_half * (highest_point - 10).max(0);
    transient_eval += board_weights.top_quarter * (highest_point - 15).max(0);

    let mut well = 0;
    for x in 1..10 {
        if board.column_heights()[x] < board.column_heights()[well] {
            well = x;
        }
    }

    let mut depth = 0;
    'yloop: for y in board.column_heights()[well] .. 20 {
        for x in 0..10 {
            if x as usize != well && !board.occupied(x, y) {
                break 'yloop;
            }
            depth += 1;
        }
    }
    let depth = depth.min(board_weights.max_well_depth);
    transient_eval += board_weights.well_depth * depth;

    if board_weights.bumpiness | board_weights.bumpiness_sq != 0 {
        let (bump, bump_sq) = bumpiness(board, well);
        transient_eval += bump * board_weights.bumpiness;
        transient_eval += bump_sq * board_weights.bumpiness_sq;
    }

    if board_weights.cavity_cells | board_weights.cavity_cells_sq |
            board_weights.overhang_cells | board_weights.overhang_cells_sq != 0 {
        let (cavity_cells, overhang_cells) = cavities_and_overhangs(board);
        transient_eval += board_weights.cavity_cells * cavity_cells;
        transient_eval += board_weights.cavity_cells_sq * cavity_cells * cavity_cells;
        transient_eval += board_weights.overhang_cells * overhang_cells;
        transient_eval += board_weights.overhang_cells_sq * overhang_cells * overhang_cells;
    }

    if board_weights.covered_cells | board_weights.covered_cells_sq != 0 {
        let (covered_cells, covered_cells_sq) = covered_cells(board);
        transient_eval += board_weights.covered_cells * covered_cells;
        transient_eval += board_weights.covered_cells_sq * covered_cells_sq;
    }

    if tslot(&board) {
        transient_eval += board_weights.tslot_present;
    }

    unsafe {
        TIME_TAKEN += t.elapsed();
        BOARDS_EVALUATED += 1;
    }

    Evaluation {
        accumulated: acc_eval,
        transient: transient_eval
    }
}

/// Evaluates the bumpiness of the playfield.
/// 
/// The first returned value is the total amount of height change outside of an apparent well. The
/// second returned value is the sum of the squares of the height changes outside of an apparent
/// well.
fn bumpiness(board: &Board, well: usize) -> (i32, i32) {
    let mut bumpiness = -1;
    let mut bumpiness_sq = -1;

    let mut prev = if well == 0 { 1 } else { 0 };
    for i in 1..10 {
        if i == well {
            continue
        }
        let dh = (board.column_heights()[prev] - board.column_heights()[i]).abs();
        bumpiness += dh;
        bumpiness_sq += dh * dh;
        prev = i;
    }

    (bumpiness.abs() as i32, bumpiness_sq.abs() as i32)
}

/// Evaluates the holes in the playfield.
/// 
/// The first returned value is the number of cells that make up fully enclosed spaces (cavities).
/// The second is the number of cells that make up partially enclosed spaces (overhangs).
fn cavities_and_overhangs(board: &Board) -> (i32, i32) {
    let mut checked = ArrayVec::from([[false; 10]; 40]);

    let mut cavity_cells = 0;
    let mut overhang_cells = 0;

    for y in 0..40 {
        for x in 0..10 {
            if board.occupied(x, y) ||
                    checked[y as usize][x as usize] ||
                    y >= board.column_heights()[x as usize] {
                continue
            }

            let mut is_overhang = false;
            let mut size = 0;
            let mut to_check = VecDeque::new();
            to_check.push_back((x, y));

            while let Some((x, y)) = to_check.pop_front() {
                if x < 0 || y < 0 || x >= 10 || y >= 40 ||
                        board.occupied(x, y) || checked[y as usize][x as usize] {
                    continue
                }

                if y >= board.column_heights()[x as usize] {
                    is_overhang = true;
                    continue
                }

                checked[y as usize][x as usize] = true;
                size += 1;

                to_check.push_back((x-1, y));
                to_check.push_back((x, y-1));
                to_check.push_back((x+1, y));
                to_check.push_back((x, y+1));
            }

            if is_overhang {
                overhang_cells += size;
            } else {
                cavity_cells += size;
            }
        }
    }

    (cavity_cells, overhang_cells)
}

/// Evaluates how covered holes in the playfield are.
/// 
/// The first returned value is the number of filled cells cover the topmost hole in the columns.
/// The second value is the sum of the squares of those values.
fn covered_cells(board: &Board) -> (i32, i32) {
    let mut covered = 0;
    let mut covered_sq = 0;

    for x in 0..10 {
        let mut cells = 0;
        for y in (0..board.column_heights()[x] as usize).rev() {
            if !board.occupied(x as i32, y as i32) {
                covered += cells;
                covered_sq += cells * cells;
                break
            }
            cells += 1;
        }
    }

    (covered, covered_sq)
}


/// Evaluates the existence of a reachable T slot on the board.
fn tslot(board: &Board) -> bool {
    if board.next_queue().take(6).all(|p| p != libtetris::Piece::T) {
        if board.hold_piece().map_or(true, |p| p != libtetris::Piece::T) {
            return false
        }
    }

    let mut left_t = libtetris::FallingPiece {
        kind: libtetris::PieceState(libtetris::Piece::T, libtetris::RotationState::West),
        tspin: libtetris::TspinStatus::None,
        x: 0,
        y: 0
    };
    let mut right_t = left_t;
    right_t.kind.1 = libtetris::RotationState::East;

    for y in 1..20 {
        'l:
        for x in 1..9 {
            left_t.x = x;
            left_t.y = y;
            right_t.x = x;
            right_t.y = y;
            if board.obstructed(&left_t) || board.obstructed(&right_t) {
                continue
            }
            if !board.occupied(x-1, y-1) || !board.occupied(x+1, y-1) {
                continue
            }
            if board.above_stack(&left_t) && board.occupied(x+1, y+1) ||
                    board.above_stack(&right_t) && board.occupied(x-1, y+1) {
                // for rx in 0..10 {
                //     if rx < x-1 || rx > x+1 {
                //         if !board.occupied(rx, y) || !board.occupied(rx, y-1) {
                //             continue 'l
                //         }
                //     }
                // }
                return true
            }
        }
    }
    false
}