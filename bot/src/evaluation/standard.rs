use libtetris::*;
use serde::{ Serialize, Deserialize };
use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Standard {
    pub back_to_back: i32,
    pub bumpiness: i32,
    pub bumpiness_sq: i32,
    pub height: i32,
    pub top_half: i32,
    pub top_quarter: i32,
    pub jeopardy: i32,
    pub cavity_cells: i32,
    pub cavity_cells_sq: i32,
    pub overhang_cells: i32,
    pub overhang_cells_sq: i32,
    pub covered_cells: i32,
    pub covered_cells_sq: i32,
    pub tslot: [i32; 4],
    pub well_depth: i32,
    pub max_well_depth: i32,
    pub well_column: [i32; 10],

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
    pub combo_garbage: i32,
    pub move_time: i32,
    pub wasted_t: i32,

    pub use_bag: bool,
    pub sub_name: Option<String>
}

impl Default for Standard {
    fn default() -> Self {
        Standard {
            back_to_back: 52,
            bumpiness: -24,
            bumpiness_sq: -7,
            height: -39,
            top_half: -150,
            top_quarter: -511,
            jeopardy: -11,
            cavity_cells: -173,
            cavity_cells_sq: -3,
            overhang_cells: -34,
            overhang_cells_sq: -1,
            covered_cells: -17,
            covered_cells_sq: -1,
            tslot: [8, 148, 192, 407],
            well_depth: 57,
            max_well_depth: 17,
            well_column: [20, 23, 20, 50, 59, 21, 59, 10, -10, 24],

            move_time: -3,
            wasted_t: -152,
            b2b_clear: 104,
            clear1: -143,
            clear2: -100,
            clear3: -58,
            clear4: 390,
            tspin1: 121,
            tspin2: 410,
            tspin3: 602,
            mini_tspin1: -158,
            mini_tspin2: -93,
            perfect_clear: 999,
            combo_garbage: 150,

            use_bag: true,
            sub_name: None
        }
    }
}

impl Standard {
    pub fn fast_config() -> Self {
        Standard {
            back_to_back: 10,
            bumpiness: -7,
            bumpiness_sq: -28,
            height: -46,
            top_half: -126,
            top_quarter: -493,
            jeopardy: -11,
            cavity_cells: -176,
            cavity_cells_sq: -6,
            overhang_cells: -47,
            overhang_cells_sq: -9,
            covered_cells: -25,
            covered_cells_sq: 1,
            tslot: [0, 150, 296, 207],
            well_depth: 158,
            max_well_depth: -2,
            well_column: [31, 16, -41, 37, 49, 30, 56, 48, -27, 22],
            b2b_clear: 74,
            clear1: -122,
            clear2: -174,
            clear3: 11,
            clear4: 424,
            tspin1: 131,
            tspin2: 392,
            tspin3: 628,
            mini_tspin1: -188,
            mini_tspin2: -682,
            perfect_clear: 991,
            combo_garbage: 272,
            move_time: -1,
            wasted_t: -147,
            use_bag: true,
            sub_name: None
        }
    }
}

impl Evaluator for Standard {
    type Value = Value;
    type Reward = Reward;

    fn name(&self) -> String {
        let mut info = "Standard".to_owned();
        if let Some(extra) = &self.sub_name {
            info.push('\n');
            info.push_str(extra);
        }
        info
    }

    fn pick_move(
        &self, candidates: Vec<MoveCandidate<Value>>, incoming: u32
    ) -> MoveCandidate<Value> {
        let mut backup = None;
        for mv in candidates.into_iter() {
            if mv.board.column_heights()[3..6].iter()
                    .all(|h| incoming as i32 - mv.lock.garbage_sent as i32 + h <= 20) {
                return mv
            }

            match backup {
                None => backup = Some(mv),
                Some(c) if c.evaluation.spike < mv.evaluation.spike => backup = Some(mv),
                _ => {}
            }
        }

        return backup.unwrap();
    }

    fn evaluate(
        &self, lock: &LockResult, board: &Board, move_time: u32, placed: Piece
    ) -> (Value, Reward) {
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
                acc_eval += self.combo_garbage * libtetris::COMBO_GARBAGE[combo] as i32;
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

        if placed == Piece::T {
            match lock.placement_kind {
                PlacementKind::Tspin1 | PlacementKind::Tspin2 | PlacementKind::Tspin3 => {}
                _ => acc_eval += self.wasted_t
            }
        }

        // magic approximation of line clear delay
        let move_time = if lock.placement_kind.is_clear() {
            move_time as i32 + 40
        } else {
            move_time as i32
        };
        acc_eval += self.move_time * move_time;

        if board.b2b_bonus {
            transient_eval += self.back_to_back;
        }

        let highest_point = *board.column_heights().iter().max().unwrap() as i32;
        transient_eval += self.top_quarter * (highest_point - 15).max(0);
        transient_eval += self.top_half * (highest_point - 10).max(0);

        acc_eval += (self.jeopardy * (highest_point - 10).max(0) * move_time) / 10;

        let ts = if self.use_bag {
            board.next_bag().contains(Piece::T) as usize
                + (board.next_bag().len() <= 3) as usize
                + (board.hold_piece == Some(Piece::T)) as usize
        } else {
            1 + (board.hold_piece == Some(Piece::T)) as usize
        };

        let mut board = board.clone();
        for _ in 0..ts {
            let result = if let Some((x, y)) = sky_tslot(&board) {
                cutout_tslot(board.clone(), FallingPiece {
                    x, y,
                    kind: PieceState(Piece::T, RotationState::South),
                    tspin: TspinStatus::Full
                })
            } else if let Some(twist) = tst_twist(&board) {
                let piece = twist.piece();
                if let Some((x, y)) = cave_tslot(&board, piece) {
                    cutout_tslot(board.clone(), FallingPiece {
                        x, y,
                        kind: PieceState(Piece::T, RotationState::South),
                        tspin: TspinStatus::Full
                    })
                } else if twist.is_tslot {
                    cutout_tslot(board.clone(), piece)
                } else {
                    break
                }
            } else if let Some(twist) = fin_to_win(&board) {
                cutout_tslot(board.clone(), twist.piece())
            } else {
                break
            };
            transient_eval += self.tslot[result.lines];
            if let Some(b) = result.result {
                board = b;
            } else {
                break
            }
        }

        let highest_point = *board.column_heights().iter().max().unwrap() as i32;
        transient_eval += self.height * highest_point;

        let mut well = 0;
        for x in 1..10 {
            if board.column_heights()[x] <= board.column_heights()[well] {
                well = x;
            }
        }

        let mut depth = 0;
        'yloop: for y in board.column_heights()[well] .. 20 {
            for x in 0..10 {
                if x as usize != well && !board.occupied(x, y) {
                    break 'yloop;
                }
            }
            depth += 1;
        }
        let depth = depth.min(self.max_well_depth);
        transient_eval += self.well_depth * depth;
        if depth != 0 {
            transient_eval += self.well_column[well];
        }

        if self.bumpiness | self.bumpiness_sq != 0 {
            let (bump, bump_sq) = bumpiness(&board, well);
            transient_eval += bump * self.bumpiness;
            transient_eval += bump_sq * self.bumpiness_sq;
        }

        if self.cavity_cells | self.cavity_cells_sq |
                self.overhang_cells | self.overhang_cells_sq != 0 {
            let (cavity_cells, overhang_cells) = cavities_and_overhangs(&board);
            transient_eval += self.cavity_cells * cavity_cells;
            transient_eval += self.cavity_cells_sq * cavity_cells * cavity_cells;
            transient_eval += self.overhang_cells * overhang_cells;
            transient_eval += self.overhang_cells_sq * overhang_cells * overhang_cells;
        }

        if self.covered_cells | self.covered_cells_sq != 0 {
            let (covered_cells, covered_cells_sq) = covered_cells(&board);
            transient_eval += self.covered_cells * covered_cells;
            transient_eval += self.covered_cells_sq * covered_cells_sq;
        }

        (Value {
            value: transient_eval,
            spike: 0
        }, Reward {
            value: acc_eval,
            attack: if lock.placement_kind.is_clear() { lock.garbage_sent as i32 } else { -1 }
        })
    }

    fn get_result(&self, v: &Self::Value) -> i32 {
        v.value
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
    let mut cavities = 0;
    let mut overhangs = 0;

    for y in 0..*board.column_heights().iter().max().unwrap() {
        for x in 0..10 {
            if board.occupied(x as i32, y) || y >= board.column_heights()[x] {
                continue
            }

            if x > 1 {
                if board.column_heights()[x-1] <= y-1 && board.column_heights()[x-2] <= y {
                    overhangs += 1;
                    continue
                }
            }

            if x < 8 {
                if board.column_heights()[x+1] <= y-1 && board.column_heights()[x+2] <= y {
                    overhangs += 1;
                    continue
                }
            }

            cavities += 1;
        }
    }

    (cavities, overhangs)
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
        'yloop: for cy in y-1..y+1 {
            for rx in 0..10 {
                if rx < x-1 || rx > x+1 {
                    if !board.occupied(rx, cy) {
                        continue 'yloop;
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

#[derive(Debug, Copy, Clone)]
struct TstTwist {
    point_left: bool,
    is_tslot: bool,
    x: i32,
    y: i32,
}

impl TstTwist {
    fn piece(&self) -> FallingPiece {
        let orientation = if self.point_left {
            RotationState::West
        } else {
            RotationState::East
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
/// []....{}    {}....[]
/// ......{}    {}......
/// ..[]            []..
/// ....            ....
/// ..                ..
/// ```
/// where the `{}` have the same occupied state
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
                !board.occupied(x, middle_h-3) &&
                board.occupied(x+3, middle_h) == board.occupied(x+3, middle_h+1);
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
                !board.occupied(x+2, middle_h-3) &&
                board.occupied(x-1, middle_h) == board.occupied(x-1, middle_h+1);
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

/// Finds this thing:
/// ```
/// ....[][]
/// ........
///   ......[]
///   []....
///     []..[]
/// ```
/// and the mirror version, with sky above.
fn fin_to_win(board: &Board) -> Option<TstTwist> {
    for x in 0..7 {
        // left-pointing fin
        let h = board.column_heights()[x as usize + 1];
        if board.column_heights()[x as usize] <= h+1 &&
                board.occupied(x+1, h-1) &&
                board.occupied(x+2, h+2) && board.occupied(x+2, h-2) &&
                board.occupied(x+3, h+2) &&
                board.occupied(x+4, h) && board.occupied(x+4, h-2) &&
                !board.occupied(x+2, h+1) && !board.occupied(x+3, h+1) &&
                !board.occupied(x+2, h) && !board.occupied(x+3, h) &&
                !board.occupied(x+2, h-1) && !board.occupied(x+3, h-1) &&
                !board.occupied(x+3, h-2) {
            return Some(TstTwist {
                point_left: true,
                is_tslot: true,
                x: x+3,
                y: h-1
            });
        }
        // right-pointing fin
        let h = board.column_heights()[x as usize + 2];
        if board.column_heights()[x as usize + 3] <= h+1 &&
                board.occupied(x, h+2) && board.occupied(x+1, h+2) &&
                board.occupied(x+1, h-2) && board.occupied(x+2, h-1) &&
                board.occupied(x-1, h) && board.occupied(x-1, h-2) &&
                !board.occupied(x, h+1) && !board.occupied(x+1, h+1) &&
                !board.occupied(x, h) && !board.occupied(x+1, h) &&
                !board.occupied(x, h-1) && !board.occupied(x+1, h-1) &&
                !board.occupied(x, h-2) {
            return Some(TstTwist {
                point_left: false,
                is_tslot: true,
                x,
                y: h-1
            });
        }
    }
    None
}

struct Cutout {
    lines: usize,
    result: Option<Board>
}

fn cutout_tslot(mut board: Board, piece: FallingPiece) -> Cutout {
    let result = if piece.kind.1 == RotationState::South {
        board.lock_piece(piece)
    } else {
        let imperial = FallingPiece {
            kind: PieceState(Piece::T, if piece.kind.1 == RotationState::East {
                RotationState::West
            } else {
                RotationState::East
            }),
            ..piece
        };
        if !board.obstructed(&imperial) && board.on_stack(&imperial) {
            board.lock_piece(imperial)
        } else if board.on_stack(&piece) {
            board.lock_piece(piece)
        } else {
            return Cutout {
                lines: 0, result: None
            };
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Reward {
    value: i32,
    attack: i32
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Default, Serialize, Deserialize)]
pub struct Value {
    value: i32,
    spike: i32
}

impl std::ops::Add for Value {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Value {
            value: self.value + rhs.value,
            spike: self.spike + rhs.spike
        }
    }
}

impl std::ops::Add<Reward> for Value {
    type Output = Self;
    fn add(self, rhs: Reward) -> Self {
        Value {
            value: self.value + rhs.value,
            spike: if rhs.attack == -1 {
                0
            } else {
                self.spike + rhs.attack
            }
        }
    }
}

impl std::ops::Div<usize> for Value {
    type Output = Self;
    fn div(self, rhs: usize) -> Self {
        Value {
            value: self.value / rhs as i32,
            spike: self.spike / rhs as i32
        }
    }
}

impl std::ops::Mul<usize> for Value {
    type Output = Self;
    fn mul(self, rhs: usize) -> Self {
        Value {
            value: self.value * rhs as i32,
            spike: self.spike * rhs as i32
        }
    }
}

impl Evaluation<Reward> for Value {
    fn modify_death(self) -> Self {
        Value {
            value: self.value - 1000,
            spike: 0
        }
    }

    fn weight(self, min: &Value, rank: usize) -> i64 {
        let e = (self.value - min.value) as i64 + 10;
        e * e / (rank*rank + 1) as i64
    }

    fn improve(&mut self, new_result: Self) {
        self.value = self.value.max(new_result.value);
        self.spike = self.spike.max(new_result.spike);
    }
}