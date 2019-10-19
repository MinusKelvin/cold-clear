use serde::{ Serialize, Deserialize };
use arrayvec::ArrayVec;
use std::collections::VecDeque;
use libtetris::*;
use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weights {
    pub back_to_back: i32,
    pub bumpiness: i32,
    pub bumpiness_sq: i32,
    pub height_before: i32,
    pub height_after: i32,
    pub cavity_cells: i32,
    pub cavity_cells_sq: i32,
    pub overhang_cells: i32,
    pub overhang_cells_sq: i32,
    pub covered_cells: i32,
    pub covered_cells_sq: i32,
    pub tslot: [i32; 4],
    pub well_depth: i32,

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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Standard {
    pub aggressive: Weights,
    pub defensive: Weights,
    pub search_options: SearchOptions,
    pub sub_name: Option<String>
}

impl Default for Standard {
    fn default() -> Self {
        Standard {
            aggressive: Weights {
                back_to_back: 50,
                bumpiness: -10,
                bumpiness_sq: -5,
                height_before: -10,
                height_after: -40,
                cavity_cells: -150,
                cavity_cells_sq: -10,
                overhang_cells: -50,
                overhang_cells_sq: -10,
                covered_cells: -8,
                covered_cells_sq: -1,
                tslot: [20, 150, 200, 400],
                well_depth: 50,

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
            },

            defensive: Weights {
                back_to_back: 50,
                bumpiness: -10,
                bumpiness_sq: -5,
                height_before: -70,
                height_after: -70,
                cavity_cells: -150,
                cavity_cells_sq: -10,
                overhang_cells: -50,
                overhang_cells_sq: -10,
                covered_cells: -8,
                covered_cells_sq: -1,
                tslot: [0, 150, 200, 400],
                well_depth: 30,

                move_time: -1,
                b2b_clear: 100,
                clear1: -100,
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
            },

            search_options: SearchOptions {
                aggressive_height: 6,
                defensive_height: 14,
                gamma: (1, 1)
            },

            sub_name: None
        }
    }
}

impl Evaluator for Standard {
    fn name(&self) -> String {
        let mut info = "Changed".to_owned();
        if let Some(extra) = &self.sub_name {
            info.push('\n');
            info.push_str(extra);
        }
        info
    }

    fn search_options(&self) -> SearchOptions {
        self.search_options
    }

    fn evaluate(
        &self, lock: &LockResult, board: &Board, move_time: u32, _: Piece
    ) -> Evaluation {
        let mut eval = Evaluation {
            aggressive_accumulated: 0,
            aggressive_transient: 0,
            defensive_accumulated: 0,
            defensive_transient: 0
        };

        if lock.perfect_clear {
            eval.aggressive_accumulated += self.aggressive.perfect_clear;
            eval.aggressive_accumulated += self.defensive.perfect_clear;
        } else {
            if lock.b2b {
                eval.aggressive_accumulated += self.aggressive.b2b_clear;
                eval.defensive_accumulated += self.defensive.b2b_clear;
            }
            if let Some(combo) = lock.combo {
                let combo = combo.min(11) as usize;
                eval.aggressive_accumulated += self.aggressive.combo_table[combo];
                eval.defensive_accumulated += self.defensive.combo_table[combo];
            }
            match lock.placement_kind {
                PlacementKind::Clear1 => {
                    eval.aggressive_accumulated += self.aggressive.clear1;
                    eval.defensive_accumulated += self.defensive.clear1;
                }
                PlacementKind::Clear2 => {
                    eval.aggressive_accumulated += self.aggressive.clear2;
                    eval.defensive_accumulated += self.defensive.clear2;
                }
                PlacementKind::Clear3 => {
                    eval.aggressive_accumulated += self.aggressive.clear3;
                    eval.defensive_accumulated += self.defensive.clear3;
                }
                PlacementKind::Clear4 => {
                    eval.aggressive_accumulated += self.aggressive.clear4;
                    eval.defensive_accumulated += self.defensive.clear4;
                }
                PlacementKind::Tspin1 => {
                    eval.aggressive_accumulated += self.aggressive.tspin1;
                    eval.defensive_accumulated += self.defensive.tspin1;
                }
                PlacementKind::Tspin2 => {
                    eval.aggressive_accumulated += self.aggressive.tspin2;
                    eval.defensive_accumulated += self.defensive.tspin2;
                }
                PlacementKind::Tspin3 => {
                    eval.aggressive_accumulated += self.aggressive.tspin3;
                    eval.defensive_accumulated += self.defensive.tspin3;
                }
                PlacementKind::MiniTspin1 => {
                    eval.aggressive_accumulated += self.aggressive.mini_tspin1;
                    eval.defensive_accumulated += self.defensive.mini_tspin1;
                }
                PlacementKind::MiniTspin2 => {
                    eval.aggressive_accumulated += self.aggressive.mini_tspin2;
                    eval.defensive_accumulated += self.defensive.mini_tspin2;
                }
                _ => {}
            }
        }

        // magic approximations of spawn delay and line clear delay
        if lock.placement_kind.is_clear() {
            eval.aggressive_accumulated += self.aggressive.move_time * (move_time + 10 + 45) as i32;
            eval.defensive_accumulated += self.defensive.move_time * (move_time + 10 + 45) as i32;
         } else {
            eval.aggressive_accumulated += self.aggressive.move_time * (move_time + 10) as i32;
            eval.defensive_accumulated += self.defensive.move_time * (move_time + 10) as i32;
        }

        if board.b2b_bonus {
            eval.aggressive_transient += self.aggressive.back_to_back;
            eval.defensive_transient += self.defensive.back_to_back;
        }

        let h = board.column_heights().iter().cloned().max().unwrap();
        eval.aggressive_transient += self.aggressive.height_before * h;
        eval.defensive_transient += self.defensive.height_before * h;

        let mut board = board.clone();
        loop {
            let result = if let Some((x, y)) = sky_tslot(&board) {
                cutout_tslot(board.clone(), x, y, TslotKind::Tsd)
            } else if let Some(twist) = tst_twist(&board) {
                let piece = twist.piece();
                if let Some((x, y)) = cave_tslot(&board, piece) {
                    cutout_tslot(board.clone(), x, y, TslotKind::Tsd)
                } else if twist.is_tslot && board.on_stack(&piece) {
                    let kind = if twist.point_left {
                        TslotKind::LeftTst
                    } else {
                        TslotKind::RightTst
                    };
                    cutout_tslot(board.clone(), twist.x, twist.y, kind)
                } else {
                    break
                }
            } else {
                break
            };
            eval.aggressive_transient += self.aggressive.tslot[result.lines];
            eval.defensive_transient += self.defensive.tslot[result.lines];
            if let Some(b) = result.result {
                board = b;
            } else {
                break
            }
        }

        let h = board.column_heights().iter().cloned().max().unwrap();
        eval.aggressive_transient += self.aggressive.height_after * h;
        eval.defensive_transient += self.defensive.height_after * h;

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
        let depth = depth.min(8);
        eval.aggressive_transient += self.aggressive.well_depth * depth;
        eval.defensive_transient += self.defensive.well_depth * depth;

        let (bump, bump_sq) = bumpiness(&board, well);
        eval.aggressive_transient += self.aggressive.bumpiness * bump;
        eval.defensive_transient += self.defensive.bumpiness * bump;
        eval.aggressive_transient += self.aggressive.bumpiness_sq * bump_sq;
        eval.defensive_transient += self.defensive.bumpiness_sq * bump_sq;

        let (cavity_cells, overhang_cells) = cavities_and_overhangs(&board);
        eval.aggressive_transient += self.aggressive.cavity_cells * cavity_cells;
        eval.defensive_transient += self.defensive.cavity_cells * cavity_cells;
        eval.aggressive_transient += self.aggressive.cavity_cells_sq * cavity_cells * cavity_cells;
        eval.defensive_transient += self.defensive.cavity_cells_sq * cavity_cells * cavity_cells;
        eval.aggressive_transient += self.aggressive.overhang_cells * overhang_cells;
        eval.defensive_transient += self.defensive.overhang_cells * overhang_cells;
        eval.aggressive_transient += self.aggressive.overhang_cells_sq * overhang_cells * overhang_cells;
        eval.defensive_transient += self.defensive.overhang_cells_sq * overhang_cells * overhang_cells;

        let (covered_cells, covered_cells_sq) = covered_cells(&board);
        eval.aggressive_transient += self.aggressive.covered_cells * covered_cells;
        eval.defensive_transient += self.defensive.covered_cells * covered_cells;
        eval.aggressive_transient += self.aggressive.covered_cells_sq * covered_cells_sq;
        eval.defensive_transient += self.defensive.covered_cells_sq * covered_cells_sq;

        eval
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
            }
        }
    }

    (covered, covered_sq)
}

/// Determines the existence and location of a reachable T slot on the board.
/// 
/// That is, it looks for these with sky above:
/// 
/// ```
/// []....    ....[]
/// ......    ......
/// []..[]    []..[]
/// ```
/// 
/// If there is more than one, this returns the one with the most lines filled.
fn sky_tslot(board: &Board) -> Option<(i32, i32)> {
    fn filledness(board: &Board, x: i32, y: i32) -> usize {
        let mut filled = 0;
        for cy in y-1..y+1 {
            for rx in 0..10 {
                if rx < x-1 || rx > x+1 {
                    if !board.occupied(rx, cy) {
                        break
                    }
                }
            }
            filled += 1;
        }
        filled
    }

    let mut best = None;
    for (x, hs) in board.column_heights().windows(2).enumerate() {
        let x = x as i32;
        let (left_h, right_h) = (hs[0], hs[1]);
        if left_h > right_h {
            // Look for topleft-open T slot
            // leftmost column is known to match, as is middle column; no need to check
            let is_tslot =
                board.occupied(x+2, left_h+1) &&
                !board.occupied(x+2, left_h) &&
                board.occupied(x+2, left_h-1);
            if is_tslot {
                best = match best {
                    None => Some((filledness(board, x+1, left_h), x+1, left_h)),
                    Some((f, ox, oy)) => {
                        let fill = filledness(board, x+1, left_h);
                        if fill > f {
                            Some((fill, x+1, left_h))
                        } else {
                            Some((f, ox, oy))
                        }
                    }
                }
            }
        } else if right_h > left_h {
            // Look for topright-open T slot
            // rightmost column is known to match, as is middle column; no need to check
            let is_tslot =
                board.occupied(x-1, right_h+1) &&
                !board.occupied(x-1, right_h) &&
                board.occupied(x-1, right_h-1);
            if is_tslot {
                best = match best {
                    None => Some((filledness(board, x, right_h), x, right_h)),
                    Some((f, ox, oy)) => {
                        let fill = filledness(board, x, right_h);
                        if fill > f {
                            Some((fill, x, right_h))
                        } else {
                            Some((f, ox, oy))
                        }
                    }
                }
            }
        } else {
            continue
        }
    }
    best.map(|(_,x,y)| (x,y))
}

fn cave_tslot(board: &Board, mut starting_point: FallingPiece) -> Option<(i32, i32)> {
    starting_point.sonic_drop(board);
    let x = starting_point.x;
    let y = starting_point.y;
    match starting_point.kind.1 {
        RotationState::East => {
            // Check:
            // []<>      <>  
            // ..<><>  []<><>[]
            // []<>[]    <>....
            //           []..[]
            if !board.occupied(x-1, y) &&
                board.occupied(x-1, y-1) &&
                board.occupied(x+1, y-1) &&
                board.occupied(x-1, y+1)
            {
                Some((x, y))
            } else if !board.occupied(x+1, y-1) &&
                !board.occupied(x+2, y-1) &&
                !board.occupied(x+1, y-2) &&
                board.occupied(x-1, y) &&
                board.occupied(x+2, y) &&
                board.occupied(x, y-2) &&
                board.occupied(x+2, y-2)
            {
                Some((x+1, y-1))
            } else {
                None
            }
        }
        RotationState::West => {
            // Check:
            //   <>[]      <>
            // <><>..  []<><>[]
            // []<>[]  ....<>
            //         []..[]
            if !board.occupied(x+1, y) &&
                board.occupied(x+1, y+1) &&
                board.occupied(x+1, y-1) &&
                board.occupied(x-1, y-1)
            {
                Some((x, y))
            } else if !board.occupied(x-1, y-1) &&
                !board.occupied(x-2, y-1) &&
                !board.occupied(x-1, y-2) &&
                board.occupied(x+1, y) &&
                board.occupied(x-2, y) &&
                board.occupied(x-2, y-2) &&
                board.occupied(x, y-2)
            {
                Some((x-1, y-1))
            } else {
                None
            }
        }
        _ => None
    }
}

struct TstTwist {
    point_left: bool,
    is_tslot: bool,
    x: i32,
    y: i32,
}

impl TstTwist {
    fn piece(&self) -> FallingPiece {
        let orientation = if self.point_left {
            RotationState::East
        } else {
            RotationState::West
        };
        FallingPiece {
            kind: PieceState(Piece::T, orientation),
            x: self.x,
            y: self.y,
            tspin: if self.is_tslot { TspinStatus::Full } else { TspinStatus::None }
        }
    }
}

/// Determines the existence and location of a reachable TST twist spot on the board.
/// 
/// That is, if looks for these with sky above:
/// 
/// ```
/// []....    ....[]
/// ......    ......
/// ..[]        []..
/// ....        ....
/// ..            ..
/// ```
fn tst_twist(board: &Board) -> Option<TstTwist> {
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
                !board.occupied(x, middle_h-3);
            if is_tst_slot {
                return Some(TstTwist {
                    point_left: false,
                    x: x,
                    y: middle_h-2,
                    is_tslot: (
                        board.occupied(x-1, middle_h-1) as usize +
                        board.occupied(x-1, middle_h-3) as usize +
                        board.occupied(x+1, middle_h-3) as usize
                    ) >= 2
                });
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
                !board.occupied(x+2, middle_h-3);
            if is_tst_slot {
                return Some(TstTwist {
                    point_left: true,
                    x: x+2,
                    y: middle_h-2,
                    is_tslot: (
                        board.occupied(x+1, middle_h-3) as usize +
                        board.occupied(x+3, middle_h-1) as usize +
                        board.occupied(x+3, middle_h-3) as usize
                    ) >= 2
                });
            }
        }
    }
    None
}

struct Cutout {
    lines: usize,
    result: Option<Board>
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum TslotKind {
    Tsd,
    LeftTst,
    RightTst
}

fn cutout_tslot(mut board: Board, x: i32, y: i32, kind: TslotKind) -> Cutout {
    let result = if kind == TslotKind::Tsd {
        board.lock_piece(FallingPiece {
            kind: PieceState(Piece::T, RotationState::South),
            x, y,
            tspin: TspinStatus::Full
        })
    } else {
        let left = FallingPiece {
            kind: PieceState(Piece::T, RotationState::East),
            x, y,
            tspin: TspinStatus::Full
        };
        let right = FallingPiece {
            kind: PieceState(Piece::T, RotationState::West),
            x, y,
            tspin: TspinStatus::Full
        };

        let (imperial, normal) = if kind == TslotKind::LeftTst {
            (right, left)
        } else {
            (left, right)
        };

        if !board.obstructed(&imperial) {
            board.lock_piece(imperial)
        } else {
            board.lock_piece(normal)
        }
    };

    match result.placement_kind {
        PlacementKind::Tspin => Cutout {
            lines: 0, result: None
        },
        PlacementKind::Tspin1 => Cutout {
            lines: 1, result: None
        },
        PlacementKind::Tspin2 => Cutout {
            lines: 2, result: Some(board)
        },
        PlacementKind::Tspin3 => Cutout {
            lines: 3, result: Some(board)
        },
        _ => unreachable!()
    }
}
