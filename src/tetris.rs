use arrayvec::ArrayVec;
use enumset::EnumSet;
use enumset::EnumSetType;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct BoardState<R=u16> {
    cells: ArrayVec<[R; 40]>,
    pub column_heights: [i32; 10],
    pub total_garbage: u32,
    pub combo: u32,
    pub b2b_bonus: bool,
    pub hold_piece: Option<Piece>,
    pub next_pieces: VecDeque<Piece>,
    pub piece_count: u32,
    pub was_soft_dropped: bool,
    pub last_lock_kind: ClearKind,
    in_bag: EnumSet<Piece>
}

pub trait Row: Default + Copy + Clone {
    fn set(&mut self, x: usize, color: CellColor);
    fn get(&self, x: usize) -> bool;
    fn is_full(&self) -> bool;
    fn cell_color(&self, x: usize) -> CellColor;
}

impl<R: Row> BoardState<R> {
    /// Creates a new empty board with no history.
    pub fn new() -> Self {
        BoardState {
            cells: [R::default(); 40].into(),
            column_heights: [0; 10],
            total_garbage: 0,
            combo: 0,
            b2b_bonus: false,
            hold_piece: None,
            next_pieces: VecDeque::new(),
            piece_count: 0,
            was_soft_dropped: false,
            last_lock_kind: ClearKind::None,
            in_bag: EnumSet::all()
        }
    }

    pub fn generate_next_piece(&self) -> Piece {
        use rand::prelude::*;
        let choices: ArrayVec<[_; 7]> = self.in_bag.iter().collect();
        *choices.choose(&mut thread_rng()).unwrap()
    }

    /// Retrieves the next piece in the queue.
    /// 
    /// If the queue is empty, returns the set of possible next pieces.
    pub fn get_next_piece(&self) -> Result<Piece, EnumSet<Piece>> {
        self.next_pieces.front().copied().ok_or(self.in_bag)
    }

    /// Retrieves the piece after the next piece in the queue if it is known.
    pub fn get_next_next_piece(&self) -> Option<Piece> {
        self.next_pieces.get(1).copied()
    }

    pub fn add_next_piece(&mut self, piece: Piece) {
        self.in_bag.remove(piece);
        if self.in_bag.is_empty() {
            self.in_bag = EnumSet::all();
        }
        self.next_pieces.push_back(piece);
    }

    fn remove_cleared_lines(&mut self) -> usize {
        self.cells.retain(|&mut r| !r.is_full());
        let cleared = 40 - self.cells.len();
        for _ in 0..cleared {
            self.cells.push(R::default());
        }
        for x in 0..10 {
            self.column_heights[x] -= cleared as i32;
            while self.column_heights[x] > 0 &&
                    !self.cells[self.column_heights[x] as usize-1].get(x) {
                self.column_heights[x] -= 1;
            }
        }
        cleared
    }

    pub fn occupied(&self, x: i32, y: i32) -> bool {
        x < 0 || y < 0 || x >= 10 || y >= 40 || (self.cells[y as usize].get(x as usize))
    }

    fn obstructed(&self, piece: &FallingPiece) -> bool {
        piece.cells()
            .into_iter()
            .any(|(x, y)| self.occupied(x, y))
    }

    pub fn above_stack(&self, piece: &FallingPiece) -> bool {
        piece.cells()
            .into_iter()
            .all(|(x, y)| y >= self.column_heights[x as usize])
    }

    /// Does all logic associated with locking a piece.
    /// 
    /// Clears lines, detects clear kind, calculates garbage, maintains combo and back-to-back
    /// state, detects perfect clears.
    pub fn lock_piece(&mut self, piece: FallingPiece) -> LockResult {
        for (x, y) in piece.cells() {
            self.cells[y as usize].set(x as usize, piece.kind.0.color());
            if self.column_heights[x as usize] < y+1 {
                self.column_heights[x as usize] = y+1;
            }
        }
        let cleared = self.remove_cleared_lines();
        let clear_kind = ClearKind::get(cleared, piece.tspin);

        let mut garbage_sent = clear_kind.base_garbage();

        let mut did_b2b = false;
        if clear_kind.is_clear() {
            if clear_kind.is_hard() {
                if self.b2b_bonus {
                    garbage_sent += 1;
                    did_b2b = true;
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
        self.piece_count += 1;
        self.was_soft_dropped = piece.soft_dropped;
        self.last_lock_kind = clear_kind;

        LockResult {
            clear_kind, garbage_sent, perfect_clear,
            combo: if self.combo == 0 { None } else { Some(self.combo-1) },
            b2b: did_b2b
        }
    }

    /// Advances the queue, and holds the piece if requested.
    /// 
    /// Returns the piece that should have been spawned, or `None` if the queue is empty.
    pub fn advance_queue(&mut self, hold: bool) -> Option<Piece> {
        self.next_pieces.pop_front()
            .and_then(|next| if hold {
                match self.hold_piece {
                    Some(hold) => {
                        self.hold_piece = Some(next);
                        Some(hold)
                    },
                    None => {
                        self.hold_piece = Some(next);
                        self.next_pieces.pop_front()
                    }
                }
            } else {
                Some(next)
            })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LockResult {
    pub clear_kind: ClearKind,
    pub b2b: bool,
    pub combo: Option<u32>,
    pub garbage_sent: u32,
    pub perfect_clear: bool
}

impl Default for LockResult {
    fn default() -> Self {
        LockResult {
            clear_kind: ClearKind::None,
            b2b: false,
            combo: None,
            garbage_sent: 0,
            perfect_clear: false
        }
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct FallingPiece {
    pub kind: PieceState,
    pub x: i32,
    pub y: i32,
    pub tspin: TspinStatus,
    pub sonic_dropped: bool,
    pub soft_dropped: bool
}

impl FallingPiece {
    fn start(kind: PieceState, x: i32) -> Self {
        FallingPiece {
            x, kind,
            y: 19,
            tspin: TspinStatus::None,
            sonic_dropped: false,
            soft_dropped: false
        }
    }

    pub fn spawn(piece: Piece, board: &BoardState) -> Option<FallingPiece> {
        let mut this = FallingPiece {
            kind: PieceState(piece, RotationState::North),
            x: 4, y: 20,
            tspin: TspinStatus::None,
            sonic_dropped: false,
            soft_dropped: false
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
            if self.sonic_dropped {
                self.soft_dropped = true;
            }
            true
        }
    }

    pub fn sonic_drop(&mut self, board: &BoardState) -> bool {
        let drop_by = self.cells()
            .into_iter()
            .map(|(x, y)| y - board.column_heights[x as usize])
            .min().unwrap();
        if drop_by > 0 {
            self.sonic_dropped = true;
            self.tspin = TspinStatus::None;
            self.y -= drop_by;
            true
        } else if drop_by < 0 {
            let mut fell = false;
            loop {
                self.y -= 1;
                if board.obstructed(self) {
                    self.y += 1;
                    break
                }
                fell = true;
                self.tspin = TspinStatus::None;
            }
            self.sonic_dropped = fell;
            fell
        } else {
            false
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
                    } else {
                        self.tspin = TspinStatus::None;
                    }
                }
                if self.sonic_dropped {
                    self.soft_dropped = true;
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum CellColor {
    I, O, T, L, J, S, Z,
    Garbage,
    Unclearable,
    Empty
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

    pub fn name(self) -> &'static str {
        match self {
            ClearKind::None       => "             ",
            ClearKind::Clear1     => "Single       ",
            ClearKind::Clear2     => "Double       ",
            ClearKind::Clear3     => "Triple       ",
            ClearKind::Clear4     => "Tetris       ",
            ClearKind::MiniTspin  => "Mini T-Spin  ",
            ClearKind::MiniTspin1 => "Mini T-Spin 1",
            ClearKind::MiniTspin2 => "Mini T-Spin 2",
            ClearKind::Tspin      => "T-Spin       ",
            ClearKind::Tspin1     => "T-Spin Single",
            ClearKind::Tspin2     => "T-Spin Double",
            ClearKind::Tspin3     => "T-Spin Triple",

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

impl Piece {
    pub fn to_char(self) -> char {
        match self {
            Piece::I => 'I',
            Piece::T => 'T',
            Piece::O => 'O',
            Piece::L => 'L',
            Piece::J => 'J',
            Piece::S => 'S',
            Piece::Z => 'Z',
        }
    }

    pub fn color(self) -> CellColor {
        match self {
            Piece::I => CellColor::I,
            Piece::T => CellColor::T,
            Piece::O => CellColor::O,
            Piece::L => CellColor::L,
            Piece::J => CellColor::J,
            Piece::S => CellColor::S,
            Piece::Z => CellColor::Z,
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

impl Row for u16 {
    fn set(&mut self, x: usize, color: CellColor) {
        *self |= 1 << x;
    }

    fn get(&self, x: usize) -> bool {
        *self & (1 << x) != 0
    }

    fn is_full(&self) -> bool {
        *self == 0b11111_11111
    }

    fn cell_color(&self, x: usize) -> CellColor {
        if self.get(x) {
            CellColor::Garbage
        } else {
            CellColor::Empty
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ColoredRow([CellColor; 10]);

impl Default for ColoredRow {
    fn default() -> Self {
        ColoredRow([CellColor::Empty; 10])
    }
}

impl Row for ColoredRow {
    fn set(&mut self, x: usize, color: CellColor) {
        self.0[x] = color;
    }

    fn get(&self, x: usize) -> bool {
        self.0[x] != CellColor::Empty
    }

    fn is_full(&self) -> bool {
        self.0.iter().all(|&c| c != CellColor::Empty)
    }

    fn cell_color(&self, x: usize) -> CellColor {
        self.0[x]
    }
}