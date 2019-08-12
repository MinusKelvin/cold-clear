use arrayvec::ArrayVec;
use std::collections::VecDeque;
use crate::tetris::{ BoardState, LockResult, ClearKind };

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Evaluation {
    pub accumulated: i64,
    pub transient: i64
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Default)]
pub struct BoardWeights {
    pub back_to_back: i64,
    pub bumpiness: i64,
    pub bumpiness_sq: i64,
    pub height: i64,
    pub top_half: i64,
    pub top_quarter: i64,
    pub cavity_cells: i64,
    pub cavity_cells_sq: i64,
    pub overhang_cells: i64,
    pub overhang_cells_sq: i64,
    pub covered_cells: i64,
    pub covered_cells_sq: i64,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Default)]
pub struct PlacementWeights {
    pub b2b_clear: i64,
    pub clear1: i64,
    pub clear2: i64,
    pub clear3: i64,
    pub clear4: i64,
    pub tspin1: i64,
    pub tspin2: i64,
    pub tspin3: i64,
    pub mini_tspin1: i64,
    pub mini_tspin2: i64,
    pub perfect_clear: i64,
    pub combo_table: [i64; 12],
    pub soft_drop: i64
}

pub static mut TIME_TAKEN: std::time::Duration = std::time::Duration::from_secs(0);
pub static mut BOARDS_EVALUATED: usize = 0;

pub fn evaluate(
    lock: &LockResult,
    board: &BoardState,
    board_weights: &BoardWeights,
    placement_weights: &PlacementWeights
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
        match lock.clear_kind {
            ClearKind::Clear1 => {
                acc_eval += placement_weights.clear1;
            }
            ClearKind::Clear2 => {
                acc_eval += placement_weights.clear2;
            }
            ClearKind::Clear3 => {
                acc_eval += placement_weights.clear3;
            }
            ClearKind::Clear4 => {
                acc_eval += placement_weights.clear4;
            }
            ClearKind::Tspin1 => {
                acc_eval += placement_weights.tspin1;
            }
            ClearKind::Tspin2 => {
                acc_eval += placement_weights.tspin2;
            }
            ClearKind::Tspin3 => {
                acc_eval += placement_weights.tspin3;
            }
            ClearKind::MiniTspin1 => {
                acc_eval += placement_weights.mini_tspin1;
            }
            ClearKind::MiniTspin2 => {
                acc_eval += placement_weights.mini_tspin2;
            }
            _ => {}
        }
    }

    if board.b2b_bonus {
        transient_eval += board_weights.back_to_back;
    }

    let highest_point = *board.column_heights.iter().max().unwrap() as i64;
    transient_eval += board_weights.height * highest_point;
    transient_eval += board_weights.top_half * (highest_point - 10).max(0);
    transient_eval += board_weights.top_quarter * (highest_point - 15).max(0);

    if board_weights.bumpiness | board_weights.bumpiness_sq != 0 {
        let (bump, bump_sq) = bumpiness(board);
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

    if board_weights.cavity_cells | board_weights.cavity_cells_sq != 0 {
        let (covered_cells, covered_cells_sq) = covered_cells(board);
        transient_eval += board_weights.covered_cells * covered_cells;
        transient_eval += board_weights.covered_cells_sq * covered_cells_sq;
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
fn bumpiness(board: &BoardState) -> (i64, i64) {
    let mut well = 0;
    for x in 1..10 {
        if board.column_heights[x] < board.column_heights[well] {
            well = x;
        }
    }

    let mut bumpiness = -1;
    let mut bumpiness_sq = -1;

    for i in 1..well {
        let dh = (board.column_heights[i-1] - board.column_heights[i]).abs();
        bumpiness += dh;
        bumpiness_sq += dh * dh;
    }

    for i in well+2..10 {
        let dh = (board.column_heights[i-1] - board.column_heights[i]).abs();
        bumpiness += dh;
        bumpiness_sq += dh * dh;
    }

    (bumpiness.abs() as i64, bumpiness_sq.abs() as i64)
}

/// Evaluates the holes in the playfield.
/// 
/// The first returned value is the number of cells that make up fully enclosed spaces (cavities).
/// The second is the number of cells that make up partially enclosed spaces (overhangs).
fn cavities_and_overhangs(board: &BoardState) -> (i64, i64) {
    let mut checked = ArrayVec::from([[false; 10]; 40]);

    let mut cavity_cells = 0;
    let mut overhang_cells = 0;

    for y in 0..40 {
        for x in 0..10 {
            if board.occupied(x, y) ||
                    checked[y as usize][x as usize] ||
                    y >= board.column_heights[x as usize] {
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

                if y >= board.column_heights[x as usize] {
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
fn covered_cells(board: &BoardState) -> (i64, i64) {
    let mut covered = 0;
    let mut covered_sq = 0;

    for x in 0..10 {
        let mut cells = 0;
        for y in (0..board.column_heights[x] as usize).rev() {
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