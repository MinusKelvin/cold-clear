use arrayvec::ArrayVec;
use std::collections::VecDeque;
use libtetris::{ Board, LockResult, PlacementKind };
use super::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NaiveEvaluator {
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
    pub tslot: [i32; 3],
    pub tst_slot: [i32; 4],
    pub well_depth: i32,
    pub max_well_depth: i32,

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

impl Default for NaiveEvaluator {
    fn default() -> Self {
        NaiveEvaluator {
            back_to_back: 50,
            bumpiness: -10,
            bumpiness_sq: -5,
            height: -40,
            top_half: -150,
            top_quarter: -500,
            cavity_cells: -150,
            cavity_cells_sq: -10,
            overhang_cells: -50,
            overhang_cells_sq: -10,
            covered_cells: -10,
            covered_cells_sq: -10,
            tslot: [50, 150, 200],
            // tst slots look *really* bad to other heuristics
            tst_slot: [500, 600, 700, 900],
            well_depth: 50,
            max_well_depth: 8,

            soft_drop: -10,
            b2b_clear: 100,
            clear1: -150,
            clear2: -100,
            clear3: -50,
            clear4: 400,
            tspin1: 130,
            tspin2: 400,
            tspin3: 600,
            mini_tspin1: 0,
            mini_tspin2: 100,
            perfect_clear: 1000,
            combo_table: libtetris::COMBO_GARBAGE.iter()
                .map(|&v| v as i32 * 100)
                .collect::<ArrayVec<[_; 12]>>()
                .into_inner()
                .unwrap()
        }
    }
}

impl Evaluator for NaiveEvaluator {
    fn info(&self) -> Info {
        vec![("Naive".to_string(), None)]
    }

    fn evaluate(&mut self, lock: &LockResult, board: &Board, soft_dropped: bool) -> Evaluation {
        let mut transient_eval = 0;
        let mut acc_eval = 0;

        if lock.perfect_clear {
            acc_eval += self.perfect_clear;
        } else {
            if lock.b2b {
                acc_eval += self.b2b_clear;
            }
            if let Some(combo) = lock.combo {
                let combo = combo.min(11) as usize;
                acc_eval += self.combo_table[combo];
            }
            match lock.placement_kind {
                PlacementKind::Clear1 => {
                    acc_eval += self.clear1;
                }
                PlacementKind::Clear2 => {
                    acc_eval += self.clear2;
                }
                PlacementKind::Clear3 => {
                    acc_eval += self.clear3;
                }
                PlacementKind::Clear4 => {
                    acc_eval += self.clear4;
                }
                PlacementKind::Tspin1 => {
                    acc_eval += self.tspin1;
                }
                PlacementKind::Tspin2 => {
                    acc_eval += self.tspin2;
                }
                PlacementKind::Tspin3 => {
                    acc_eval += self.tspin3;
                }
                PlacementKind::MiniTspin1 => {
                    acc_eval += self.mini_tspin1;
                }
                PlacementKind::MiniTspin2 => {
                    acc_eval += self.mini_tspin2;
                }
                _ => {}
            }
        }

        if soft_dropped {
            acc_eval += self.soft_drop;
        }

        if board.b2b_bonus {
            transient_eval += self.back_to_back;
        }

        let highest_point = *board.column_heights().iter().max().unwrap() as i32;
        transient_eval += self.height * highest_point;
        transient_eval += self.top_half * (highest_point - 10).max(0);
        transient_eval += self.top_quarter * (highest_point - 15).max(0);

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
        let depth = depth.min(self.max_well_depth);
        transient_eval += self.well_depth * depth;

        if self.bumpiness | self.bumpiness_sq != 0 {
            let (bump, bump_sq) = bumpiness(board, well);
            transient_eval += bump * self.bumpiness;
            transient_eval += bump_sq * self.bumpiness_sq;
        }

        if self.cavity_cells | self.cavity_cells_sq |
                self.overhang_cells | self.overhang_cells_sq != 0 {
            let (cavity_cells, overhang_cells) = cavities_and_overhangs(board);
            transient_eval += self.cavity_cells * cavity_cells;
            transient_eval += self.cavity_cells_sq * cavity_cells * cavity_cells;
            transient_eval += self.overhang_cells * overhang_cells;
            transient_eval += self.overhang_cells_sq * overhang_cells * overhang_cells;
        }

        if self.covered_cells | self.covered_cells_sq != 0 {
            let (covered_cells, covered_cells_sq) = covered_cells(board);
            transient_eval += self.covered_cells * covered_cells;
            transient_eval += self.covered_cells_sq * covered_cells_sq;
        }

        if let Some(filled) = tslot(board) {
            transient_eval += self.tslot[filled];
        }

        if let Some(filled) = tst_slot(board) {
            transient_eval += self.tst_slot[filled];
        }

        Evaluation {
            accumulated: acc_eval,
            transient: transient_eval
        }
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


/// Evaluates the existence and filledness of a reachable T slot on the board.
fn tslot(board: &Board) -> Option<usize> {
    let mut left_t = libtetris::FallingPiece {
        kind: libtetris::PieceState(libtetris::Piece::T, libtetris::RotationState::West),
        tspin: libtetris::TspinStatus::None,
        x: 0,
        y: 0
    };
    let mut right_t = left_t;
    right_t.kind.1 = libtetris::RotationState::East;

    let mut best = None;
    for y in 0..20 {
        'l:
        for x in 1..9 {
            left_t.x = x;
            left_t.y = y+1;
            right_t.x = x;
            right_t.y = y+1;
            if board.obstructed(&left_t) || board.obstructed(&right_t) {
                continue
            }
            if !board.occupied(x-1, y) || !board.occupied(x+1, y) {
                continue
            }
            if board.above_stack(&left_t) && board.occupied(x+1, y+2) ||
                    board.above_stack(&right_t) && board.occupied(x-1, y+2) {
                let mut filled = 2;
                for cy in y..y+2 {
                    for rx in 0..10 {
                        if rx < x-1 || rx > x+1 {
                            if !board.occupied(rx, cy) {
                                filled -= 1;
                                break
                            }
                        }
                    }
                }
                best = match best {
                    Some(fill) => Some(filled.max(fill)),
                    None => Some(filled)
                };
            }
        }
    }
    best
}

/// Evaluates the existence and filledness of a reachable TST slot on the board.
fn tst_slot(board: &Board) -> Option<usize> {
    let mut best = None;
    for y in 0..20 {
        for x in 0..10 {
            // Require 4 vertical empty cells with one solid cell above
            if board.occupied(x, y) || board.occupied(x, y+1) ||
                    board.occupied(x, y+2) || board.occupied(x, y+3) ||
                    !board.occupied(x, y+4){
                continue
            }

            // Check for corners of T slot
            let corners = board.occupied(x-1, y) as u32 +
                    board.occupied(x+1, y) as u32 +
                    board.occupied(x-1, y+2) as u32 +
                    board.occupied(x+1, y+2) as u32;
            if corners < 3 {
                continue
            }

            // Check filledness
            let mut filled = 0;
            'fillcheckloop:
            for cy in y..y+3 {
                for rx in 0..10 {
                    if rx < x-1 || rx > x+1 {
                        if !board.occupied(rx, cy) {
                            break 'fillcheckloop
                        }
                    }
                }
                filled += 1;
            }

            // Check for left side TST
            if !board.occupied(x-1, y+1) && board.occupied(x-1, y+2) && x >= 2 {
                if board.column_heights()[x as usize - 1] > y+3 ||
                        board.column_heights()[x as usize - 2] > y+3 {
                    // TST slot not reachable
                } else {
                    best = match best {
                        Some(fill) => Some(filled.max(fill)),
                        None => Some(filled)
                    };
                }
            }

            // Check for right side TST
            if  !board.occupied(x+1, y+1) && board.occupied(x+1, y+2) && x < 8 {
                if board.column_heights()[x as usize + 1] > y+3 ||
                        board.column_heights()[x as usize + 2] > y+3 {
                    // TST slot not reachable
                } else {
                    best = match best {
                        Some(fill) => Some(filled.max(fill)),
                        None => Some(filled)
                    };
                }
            }
        }
    }
    best
}
