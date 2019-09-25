use arrayvec::ArrayVec;
use enumset::EnumSetType;
use enum_map::Enum;
use serde::{ Serialize, Deserialize };

use crate::{ Board, Row };

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct FallingPiece {
    pub kind: PieceState,
    pub x: i32,
    pub y: i32,
    pub tspin: TspinStatus
}

impl FallingPiece {
    pub fn spawn<R: Row>(piece: Piece, board: &Board<R>) -> Option<FallingPiece> {
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

    pub fn shift<R: Row>(&mut self, board: &Board<R>, dx: i32, dy: i32) -> bool {
        self.x += dx;
        self.y += dy;
        if board.obstructed(self) {
            self.x -= dx;
            self.y -= dy;
            false
        } else {
            self.tspin = TspinStatus::None;
            true
        }
    }

    pub fn sonic_drop<R: Row>(&mut self, board: &Board<R>) -> bool {
        let drop_by = self.cells()
            .into_iter()
            .map(|(x, y)| y - board.column_heights()[x as usize])
            .min().unwrap();
        if drop_by > 0 {
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
            fell
        } else {
            false
        }
    }

    fn rotate<R: Row>(&mut self, target: PieceState, board: &Board<R>) -> bool {
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
                            // That guideline contains no examples of this, and this isn't the case
                            // in recent guideline games such as Puyo Puyo Tetris.
                            // For now, we won't implement it.
                            
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
                return true
            }
        }
        
        *self = initial;
        false
    }

    pub fn cw<R: Row>(&mut self, board: &Board<R>) -> bool {
        let mut target = self.kind;
        target.cw();
        self.rotate(target, board)
    }

    pub fn ccw<R: Row>(&mut self, board: &Board<R>) -> bool {
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

#[derive(Debug, Hash, EnumSetType, Enum, Serialize, Deserialize)]
pub enum Piece {
    I, O, T, L, J, S, Z
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum RotationState {
    North, South, East, West
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct PieceState(pub Piece, pub RotationState);

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum TspinStatus {
    None,
    Mini,
    Full,
    PersistentFull
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

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum PieceMovement {
    Left,
    Right,
    Cw,
    Ccw,
    SonicDrop
}

impl PieceMovement {
    pub fn apply(self, piece: &mut FallingPiece, board: &Board) -> bool {
        match self {
            PieceMovement::Left => piece.shift(board, -1, 0),
            PieceMovement::Right => piece.shift(board, 1, 0),
            PieceMovement::Ccw => piece.ccw(board),
            PieceMovement::Cw => piece.cw(board),
            PieceMovement::SonicDrop => piece.sonic_drop(board)
        }
    }
}