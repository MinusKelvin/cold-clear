use arrayvec::ArrayVec;
use std::collections::VecDeque;
use libtetris::{ Board, LockResult, PlacementKind };
use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Standard {
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
    pub move_time: i32,

    pub sub_name: Option<String>
}

impl Default for Standard {
    fn default() -> Self {
        Standard {
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

            move_time: -1,
            b2b_clear: 100,
            clear1: -150,
            clear2: -100,
            clear3: -50,
            clear4: 400,
            tspin1: 130,
            tspin2: 400,
            tspin3: 600,
            mini_tspin1: -150,
            mini_tspin2: -100,
            perfect_clear: 1000,
            combo_table: libtetris::COMBO_GARBAGE.iter()
                .map(|&v| v as i32 * 100)
                .collect::<ArrayVec<[_; 12]>>()
                .into_inner()
                .unwrap(),

            sub_name: None
        }
    }
}

impl Evaluator for Standard {
    fn name(&self) -> String {
        let mut info = "Standard".to_owned();
        if let Some(extra) = &self.sub_name {
            info.push('\n');
            info.push_str(extra);
        }
        info
    }

    fn evaluate(&self, lock: &LockResult, board: &Board, move_time: u32) -> Evaluation {
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

        // magic approximations of spawn delay and line clear delay
        acc_eval += if lock.placement_kind.is_clear() {
            self.move_time * (move_time + 10 + 45) as i32
         } else {
            self.move_time * (move_time + 10) as i32
         };

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
                    if x >= 1 {
                        is_overhang |= y >= board.column_heights()[x as usize - 1];
                    }
                    if x < 9 {
                        is_overhang |= y >= board.column_heights()[x as usize + 1];
                    }
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
        for y in (0..board.column_heights()[x] - 2).rev() {
            if !board.occupied(x as i32, y) {
                let cells = 6.min(board.column_heights()[x] - y - 1);
                covered += cells;
                covered_sq += cells * cells;
                break
            }
        }
    }

    (covered, covered_sq)
}


/// Evaluates the existence and filledness of a reachable T slot on the board.
/// That is, it looks for these with sky above:
/// 
/// ```
/// []....    ....[]
/// ......    ......
/// []..[]    []..[]
/// ```
fn tslot(board: &Board) -> Option<usize> {
    let mut best = None;
    for (x, hs) in board.column_heights().windows(2).enumerate() {
        let x = x as i32;
        let (left_h, right_h) = (hs[0], hs[1]);
        let is_tslot = if left_h > right_h {
            // Look for topleft-open T slot
            // leftmost column is known to match, as is middle column; no need to check
            board.occupied(x+2, left_h+1) &&
                !board.occupied(x+2, left_h) &&
                board.occupied(x+2, left_h-1)
        } else if right_h > left_h {
            // Look for topright-open T slot
            // rightmost column is known to match, as is middle column; no need to check
            board.occupied(x-1, right_h+1) &&
                !board.occupied(x-1, right_h) &&
                board.occupied(x-1, right_h-1)
        } else {
            false
        };

        if is_tslot {
            let y = left_h.max(right_h) - 1;
            let mut filled = 0;
            for cy in y..y+2 {
                for rx in 0..10 {
                    if rx < x || rx > x+2 {
                        if !board.occupied(rx, cy) {
                            break
                        }
                    }
                }
                filled += 1;
            }
            best = Some(filled.max(best.unwrap_or(0)));
        }
    }
    best
}

/// Evaluates the existence and filledness of a reachable TST slot on the board.
/// That is, if looks for these with sky above:
/// 
/// ```
///   []....    ....[]
///   ......    ......
/// {}..[]        []..{}
///   ....        ....
/// {}..{}        {}..{}
/// ```
/// where at least two of the `{}` cells are filled
fn tst_slot(board: &Board) -> Option<usize> {
    fn filled_check(board: &Board, x: i32, y: i32) -> usize {
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
        filled
    }

    let mut best = None;
    for (x, hs) in board.column_heights().windows(3).enumerate() {
        let x = x as i32;
        let (left_h, middle_h, right_h) = (hs[0], hs[1], hs[2]);
        if left_h > middle_h && middle_h >= right_h {
            // right-pointing TST slot
            // only really know that rightmost column and the lower pivot block match
            let is_tst_slot =
                board.occupied(x, middle_h+1) &&
                !board.occupied(x, middle_h) &&
                !board.occupied(x, middle_h-1) &&
                !board.occupied(x, middle_h-2) &&
                !board.occupied(x+1, middle_h-2) &&
                !board.occupied(x, middle_h-3) && (
                    board.occupied(x-1, middle_h-1) as usize +
                    board.occupied(x-1, middle_h-3) as usize +
                    board.occupied(x+1, middle_h-3) as usize
                ) >= 2;
            if is_tst_slot {
                best = Some(filled_check(board, x, middle_h-3).max(best.unwrap_or(0)))
            }
        } else if right_h > middle_h && middle_h >= left_h {
            // left-pointing TST slot
            // only really know that rightmost column and the lower pivot block match
            let is_tst_slot =
                board.occupied(x+2, middle_h+1) &&
                !board.occupied(x+2, middle_h) &&
                !board.occupied(x+2, middle_h-1) &&
                !board.occupied(x+2, middle_h-2) &&
                !board.occupied(x+1, middle_h-2) &&
                !board.occupied(x+2, middle_h-3) && (
                    board.occupied(x+1, middle_h-3) as usize +
                    board.occupied(x+3, middle_h-1) as usize +
                    board.occupied(x+3, middle_h-3) as usize
                ) >= 2;
            if is_tst_slot {
                best = Some(filled_check(board, x+2, middle_h-3).max(best.unwrap_or(0)))
            }
        }
    }
    best
}
