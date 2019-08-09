use arrayvec::ArrayVec;
use enumset::EnumSet;
use enumset::EnumSetType;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct BoardState {
    cells: ArrayVec<[[bool; 10]; 40]>,
    pub column_heights: [i32; 10],
    pub total_garbage: u32,
    pub combo: u32,
    pub b2b_bonus: bool,
    pub hold_piece: Option<Piece>,
    pub next_pieces: VecDeque<Piece>,
    in_bag: EnumSet<Piece>
}

impl BoardState {
    /// Creates a new empty board with no history.
    pub fn new() -> Self {
        BoardState {
            cells: [[false; 10]; 40].into(),
            column_heights: [0; 10],
            total_garbage: 0,
            combo: 0,
            b2b_bonus: false,
            hold_piece: None,
            next_pieces: VecDeque::new(),
            in_bag: EnumSet::all()
        }
    }

    /// Returns the set of possible pieces that will spawn next.
    /// 
    /// If the next queue is nonempty, the set will contain only the first piece of the next queue.
    pub fn get_next_piece_possiblities(&self) -> EnumSet<Piece> {
        self.next_pieces.front().map_or(self.in_bag, |&p| EnumSet::only(p))
    }

    fn remove_cleared_lines(&mut self) -> usize {
        self.cells.retain(|r| !r.iter().all(|&b| b));
        let cleared = 40 - self.cells.len();
        for _ in 0..cleared {
            self.cells.push([false; 10]);
        }
        cleared
    }

    pub fn occupied(&self, x: i32, y: i32) -> bool {
        x < 0 || y < 0 || x >= 10 || y >= 40 || self.cells[y as usize][x as usize]
    }

    fn obstructed(&self, piece: &FallingPiece) -> bool {
        piece.cells()
            .into_iter()
            .any(|(x, y)| self.occupied(x, y))
    }

    /// Does all logic associated with locking a piece.
    /// 
    /// Clears lines, detects clear kind, calculates garbage, maintains combo and back-to-back
    /// state, detects perfect clears.
    pub fn lock_piece(&mut self, piece: FallingPiece) -> LockResult {
        for (x, y) in piece.cells() {
            self.cells[y as usize][x as usize] = true;
            if self.column_heights[x as usize] < y+1 {
                self.column_heights[x as usize] = y+1;
            }
        }
        let cleared = self.remove_cleared_lines();
        for x in 0..10 {
            self.column_heights[x] -= cleared as i32;
        }
        let clear_kind = ClearKind::get(cleared, piece.tspin);

        let mut garbage_sent = clear_kind.base_garbage();

        if clear_kind.is_clear() {
            if clear_kind.is_hard() {
                if self.b2b_bonus {
                    garbage_sent += 1;
                }
                self.b2b_bonus = true;
            } else {
                self.b2b_bonus = false;
            }

            if self.combo as usize >= COMBO_GARBAGE.len() {
                garbage_sent += COMBO_GARBAGE.last().unwrap();
            } else {
                garbage_sent += COMBO_GARBAGE[self.combo as usize];
            }

            self.combo += 1;
        } else {
            self.combo = 0;
        }

        // It's impossible to float a mino above an empty row, so we only need to check
        // if the bottommost row is empty to determine if a perfect clear happened.
        let perfect_clear = self.column_heights.iter().all(|&y| y == 0);
        if perfect_clear {
            garbage_sent = 10;
        }

        self.total_garbage += garbage_sent;

        LockResult {
            clear_kind, garbage_sent, perfect_clear
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LockResult {
    pub clear_kind: ClearKind,
    pub garbage_sent: u32,
    pub perfect_clear: bool
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct FallingPiece {
    pub kind: PieceState,
    pub x: i32,
    pub y: i32,
    pub tspin: TspinStatus
}

impl FallingPiece {
    pub fn spawn(piece: Piece, board: &BoardState) -> Option<FallingPiece> {
        let mut this = FallingPiece {
            kind: PieceState(piece, RotationState::North),
            x: 4, y: 20,
            tspin: TspinStatus::None
        };

        if board.obstructed(&this) {
            None
        } else {
            this.y -= 1;
            if board.obstructed(&this) {
                this.y += 1;
            }

            Some(this)
        }
    }

    pub fn cells(&self) -> ArrayVec<[(i32, i32); 4]> {
        let x = self.x;
        let y = self.y;
        self.kind.cells()
            .into_iter()
            .map(move |(dx, dy)| (x + dx, y + dy))
            .collect()
    }

    pub fn shift(&mut self, board: &BoardState, dx: i32) -> bool {
        self.x += dx;
        if board.obstructed(self) {
            self.x -= dx;
            false
        } else {
            self.tspin = TspinStatus::None;
            true
        }
    }

    pub fn sonic_drop(&mut self, board: &BoardState) -> bool {
        let mut fell = false;
        loop {
            self.y -= 1;
            if board.obstructed(self) {
                self.y += 1;
                return fell;
            }
            fell = true;
            self.tspin = TspinStatus::None;
        }
    }

    fn rotate(&mut self, target: PieceState, board: &BoardState) -> bool {
        let initial = *self;
        self.kind = target;
        let kicks = initial.kind.rotation_points().into_iter()
            .zip(target.rotation_points().into_iter())
            .map(|((x1, y1), (x2, y2))| (x1 - x2, y1 - y2));

        for (i, (dx, dy)) in kicks.enumerate() {
            self.x = initial.x + dx;
            self.y = initial.y + dy;
            if !board.obstructed(self) {
                if target.0 == Piece::T && self.tspin != TspinStatus::PersistentFull {
                    let mut mini_corners = 0;
                    for (dx, dy) in target.1.mini_tspin_corners() {
                        if board.occupied(self.x + dx, self.y + dy) {
                            mini_corners += 1;
                        }
                    }

                    let mut non_mini_corners = 0;
                    for (dx, dy) in target.1.non_mini_tspin_corners() {
                        if board.occupied(self.x + dx, self.y + dy) {
                            non_mini_corners += 1;
                        }
                    }

                    if non_mini_corners + mini_corners >= 3 {
                        if i == 4 {
                            // Rotation point 5 is never a Mini T-Spin

                            // The leaked 2009 guideline says that rotations made after using the
                            // TST twist stay as full tspins, not minis. Example:
                            // http://harddrop.com/fumen/?v115@4gB8IeA8CeE8AeH8CeG8BeD8JeVBnvhC9rflrBAAA
                            // That guideline contains no examples of this, and I don't know if it
                            // is in fact the case in e.g. Puyo Puyo Tetris.
                            // For now, we will ignore this rule.
                            // self.tspin = TspinStatus::PersistentFull;
                            self.tspin = TspinStatus::Full;
                        } else if mini_corners == 2 {
                            self.tspin = TspinStatus::Full;
                        } else {
                            self.tspin = TspinStatus::Mini;
                        }
                    }
                }
                return true
            }
        }
        
        *self = initial;
        false
    }

    pub fn cw(&mut self, board: &BoardState) -> bool {
        let mut target = self.kind;
        target.cw();
        self.rotate(target, board)
    }

    pub fn ccw(&mut self, board: &BoardState) -> bool {
        let mut target = self.kind;
        target.ccw();
        self.rotate(target, board)
    }
}

#[derive(Debug, Hash, EnumSetType)]
pub enum Piece {
    I, O, T, L, J, S, Z
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum RotationState {
    North, South, East, West
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct PieceState(pub Piece, pub RotationState);

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum TspinStatus {
    None,
    Mini,
    Full,
    PersistentFull
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum ClearKind {
    None,
    Clear1,
    Clear2,
    Clear3,
    Clear4,
    MiniTspin,
    MiniTspin1,
    MiniTspin2,
    Tspin,
    Tspin1,
    Tspin2,
    Tspin3
}

impl RotationState {
    pub fn cw(&mut self) {
        use RotationState::*;
        match self {
            North => *self = East,
            East  => *self = South,
            South => *self = West,
            West  => *self = North
        }
    }
    
    pub fn ccw(&mut self) {
        use RotationState::*;
        match self {
            North => *self = West,
            West  => *self = South,
            South => *self = East,
            East  => *self = North
        }
    }

    pub fn mini_tspin_corners(self) -> ArrayVec<[(i32, i32); 2]> {
        use RotationState::*;
        match self {
            North => [(-1, 1),  (1, 1)],
            East  => [(1, 1),   (1, -1)],
            South => [(1, -1),  (-1, -1)],
            West  => [(-1, -1), (-1, 1)]
        }.into()
    }

    pub fn non_mini_tspin_corners(self) -> ArrayVec<[(i32, i32); 2]> {
        use RotationState::*;
        match self {
            South => [(-1, 1),  (1, 1)],
            West  => [(1, 1),   (1, -1)],
            North => [(1, -1),  (-1, -1)],
            East  => [(-1, -1), (-1, 1)]
        }.into()
    }
}

impl PieceState {
    pub fn cw(&mut self) {
        self.1.cw()
    }

    pub fn ccw(&mut self) {
        self.1.ccw()
    }

    /// Returns the cells this piece and orientation occupy relative
    /// to rotation point 1, in no particular order.
    pub fn cells(&self) -> ArrayVec<[(i32, i32); 4]> {
        use Piece::*;
        use RotationState::*;
        match (self.0, self.1) {
            (I, North) => [(-1, 0),  (0, 0),  (1, 0),  (2, 0)],
            (I, East)  => [(1, -2),  (1, -1), (1, 0),  (1, 1)],
            (I, South) => [(-1, -1), (0, -1), (1, -1), (2, -1)],
            (I, West)  => [(0, -2),  (0, -1), (0, 0),  (0, 1)],
            
            (O, _) => [(0, 0), (0, 1), (1, 0), (1, 1)],

            (T, North) => [(-1, 0), (0, 0), (1, 0),  (0, 1)],
            (T, East)  => [(0, 1),  (0, 0), (0, -1), (1, 0)],
            (T, South) => [(1, 0),  (0, 0), (-1, 0), (0, -1)],
            (T, West)  => [(0, -1), (0, 0), (0, 1),  (-1, 0)],

            (L, North) => [(-1, 0), (0, 0), (1, 0),  (1, 1)],
            (L, East)  => [(0, 1),  (0, 0), (0, -1), (1, -1)],
            (L, South) => [(1, 0),  (0, 0), (-1, 0), (-1, -1)],
            (L, West)  => [(0, -1), (0, 0), (0, 1),  (-1, 1)],

            (J, North) => [(-1, 0), (0, 0), (1, 0),  (-1, 1)],
            (J, East)  => [(0, 1),  (0, 0), (0, -1), (1, 1)],
            (J, South) => [(1, 0),  (0, 0), (-1, 0), (1, -1)],
            (J, West)  => [(0, -1), (0, 0), (0, 1),  (-1, -1)],

            (S, North) => [(0, 0),  (0, 1),  (-1, 0),  (1, 1)],
            (S, East)  => [(0, 0),  (1, 0),  (0, 1),   (1, -1)],
            (S, South) => [(0, -1), (0, 0),  (-1, -1), (1, 0)],
            (S, West)  => [(-1, 0), (0, 0),  (-1, 1),  (0, -1)],

            (Z, North) => [(0, 0),  (0, 1),  (-1, 1), (1, 0)],
            (Z, East)  => [(0, 0),  (1, 0),  (1, 1),  (0, -1)],
            (Z, South) => [(0, -1), (0, 0),  (-1, 0), (1, -1)],
            (Z, West)  => [(-1, 0), (0, 0),  (0, 1),  (-1, -1)],
        }.into()
    }

    /// Returns the five rotation points associated with this piece and orientation.
    /// 
    /// Note that the first point is always (0, 0). We include it here to make
    /// looping over the possible kicks easier.
    pub fn rotation_points(&self) -> ArrayVec<[(i32, i32); 5]> {
        use Piece::*;
        use RotationState::*;
        match (self.0, self.1) {
            (O, _) => [(0, 0); 5],

            (I, North) => [(0, 0), (-1, 0), (2, 0), (-1, 0), (2, 0)],
            (I, East)  => [(0, 0), (1, 0), (1, 0), (1, 1), (1, -2)],
            (I, South) => [(0, 0), (2, 0), (-1, 0), (2, -1), (-1, -1)],
            (I, West)  => [(0, 0), (0, 0), (0, 0), (0, -2), (0, 1)],

            // The rotation points for T, L, J, S, Z are all the same.
            (_, North) => [(0, 0); 5],
            (_, East)  => [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
            (_, South) => [(0, 0); 5],
            (_, West)  => [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)]
        }.into()
    }
}

impl ClearKind {
    pub fn base_garbage(self) -> u32 {
        match self {
            ClearKind::None | ClearKind::MiniTspin | ClearKind::Tspin
            | ClearKind::Clear1 | ClearKind::MiniTspin1 => 0,
            ClearKind::Clear2 | ClearKind::MiniTspin2 => 1,
            ClearKind::Clear3 | ClearKind::Tspin1 => 2,
            ClearKind::Clear4 | ClearKind::Tspin2 => 4,
            ClearKind::Tspin3 => 6
        }
    }

    pub fn is_hard(self) -> bool {
        match self {
            ClearKind::Clear4
            | ClearKind::MiniTspin1 | ClearKind::MiniTspin2
            | ClearKind::Tspin1 | ClearKind::Tspin2 | ClearKind::Tspin3 => true,
            _ => false
        }
    }

    pub fn is_clear(self) -> bool {
        match self {
            ClearKind::None | ClearKind::MiniTspin | ClearKind::Tspin => false,
            _ => true
        }
    }

    pub fn get(cleared: usize, tspin: TspinStatus) -> Self {
        match (cleared, tspin) {
            (0, TspinStatus::None) => ClearKind::None,
            (0, TspinStatus::Mini) => ClearKind::MiniTspin,
            (0, _)                 => ClearKind::Tspin,
            (1, TspinStatus::None) => ClearKind::Clear1,
            (1, TspinStatus::Mini) => ClearKind::MiniTspin1,
            (1, _)                 => ClearKind::Tspin1,
            (2, TspinStatus::None) => ClearKind::Clear2,
            (2, TspinStatus::Mini) => ClearKind::MiniTspin2,
            (2, _)                 => ClearKind::Tspin2,
            (3, TspinStatus::None) => ClearKind::Clear3,
            (3, TspinStatus::Mini) => unreachable!(),
            (3, _)                 => ClearKind::Tspin3,
            (4, TspinStatus::None) => ClearKind::Clear4,
            _ => unreachable!()
        }
    }
}

impl rand::distributions::Distribution<Piece> for rand::distributions::Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Piece {
        match rng.gen_range(0, 7) {
            0 => Piece::I,
            1 => Piece::T,
            2 => Piece::O,
            3 => Piece::L,
            4 => Piece::J,
            5 => Piece::S,
            6 => Piece::Z,
            _ => unreachable!()
        }
    }
}

const COMBO_GARBAGE: &[u32] = &[
    0,
    0,
    1,
    1,
    2,
    2,
    3,
    3,
    4,
    4,
    4,
    5
];