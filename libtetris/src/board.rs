use std::collections::VecDeque;
use std::iter::DoubleEndedIterator;

use arrayvec::ArrayVec;
use enumset::EnumSet;
use serde::{Deserialize, Serialize};

use crate::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Board<R = u16> {
    cells: ArrayVec<[R; 40]>,
    column_heights: [i32; 10],
    pub combo: u32,
    #[cfg(feature = "tetrio_garbage")]
    pub b2b_bonus: u32,
    #[cfg(not(feature = "tetrio_garbage"))]
    pub b2b_bonus: bool,
    pub hold_piece: Option<Piece>,
    next_pieces: VecDeque<Piece>,
    pub bag: EnumSet<Piece>,
}

pub trait Row: Copy + Clone + 'static {
    fn set(&mut self, x: usize, color: CellColor);
    fn get(&self, x: usize) -> bool;
    fn is_full(&self) -> bool;
    fn is_empty(&self) -> bool;
    fn cell_color(&self, x: usize) -> CellColor;

    const EMPTY: &'static Self;
    const SOLID: &'static Self;
}

impl<R: Row> Board<R> {
    /// Creates a blank board with an empty queue.
    pub fn new() -> Self {
        Board {
            cells: [*R::EMPTY; 40].into(),
            column_heights: [0; 10],
            combo: 0,
            #[cfg(feature = "tetrio_garbage")]
            b2b_bonus: 0,
            #[cfg(not(feature = "tetrio_garbage"))]
            b2b_bonus: false,
            hold_piece: None,
            next_pieces: VecDeque::new(),
            bag: EnumSet::all(),
        }
    }

    /// Creates a board with existing field, remain pieces in the bag, hold piece, back-to-back status and combo count.
    pub fn new_with_state(
        field: [[bool; 10]; 40],
        bag_remain: EnumSet<Piece>,
        hold: Option<Piece>,
        #[cfg(feature = "tetrio_garbage")] b2b: u32,
        #[cfg(not(feature = "tetrio_garbage"))] b2b: bool,
        combo: u32,
    ) -> Self {
        let mut board = Board {
            cells: [*R::EMPTY; 40].into(),
            column_heights: [0; 10],
            combo,
            b2b_bonus: b2b,
            hold_piece: hold,
            next_pieces: VecDeque::new(),
            bag: if bag_remain.is_empty() {
                EnumSet::all()
            } else {
                bag_remain
            },
        };
        board.set_field(field);
        board
    }

    /// Randomly selects a piece from the bag.
    ///
    /// This function does not remove the generated piece from the bag.
    /// Use add_next_piece() to add it to the queue.
    pub fn generate_next_piece(&self, rng: &mut impl rand::Rng) -> Piece {
        use rand::prelude::*;
        let choices: ArrayVec<[_; 7]> = self.bag.iter().collect();
        *choices.choose(rng).unwrap()
    }

    /// Retrieves the next piece in the queue.
    ///
    /// If the queue is empty, returns the set of possible next pieces.
    pub fn get_next_piece(&self) -> Result<Piece, EnumSet<Piece>> {
        self.next_pieces.front().copied().ok_or(self.bag)
    }

    /// Retrieves the piece after the next piece in the queue if it is known.
    pub fn get_next_next_piece(&self) -> Option<Piece> {
        self.next_pieces.get(1).copied()
    }

    /// Adds the piece to the next queue and removes it from the bag.
    ///
    /// If the bag becomes empty, the bag is refilled.
    pub fn add_next_piece(&mut self, piece: Piece) {
        self.bag.remove(piece);
        if self.bag.is_empty() {
            self.bag = EnumSet::all();
        }
        self.next_pieces.push_back(piece);
    }

    fn remove_cleared_lines(&mut self) -> ArrayVec<[i32; 4]> {
        let mut cleared = ArrayVec::new();
        let mut lineno = 0;
        self.cells.retain(|r| {
            let full = r.is_full();
            if full {
                cleared.push(lineno);
            }
            lineno += 1;
            !full
        });

        for _ in 0..cleared.len() {
            self.cells.push(*R::EMPTY);
        }
        for x in 0..10 {
            self.column_heights[x] -= cleared.len() as i32;
            while self.column_heights[x] > 0
                && !self.cells[self.column_heights[x] as usize - 1].get(x)
            {
                self.column_heights[x] -= 1;
            }
        }
        cleared
    }

    pub fn occupied(&self, x: i32, y: i32) -> bool {
        x < 0 || y < 0 || x >= 10 || y >= 40 || (self.cells[y as usize].get(x as usize))
    }

    pub fn get_row(&self, y: i32) -> &R {
        if y < 0 {
            R::SOLID
        } else if y >= 40 {
            R::EMPTY
        } else {
            &self.cells[y as usize]
        }
    }

    pub fn obstructed(&self, piece: &FallingPiece) -> bool {
        piece.cells().iter().any(|&(x, y)| self.occupied(x, y))
    }

    pub fn above_stack(&self, piece: &FallingPiece) -> bool {
        piece
            .cells()
            .iter()
            .all(|&(x, y)| y >= self.column_heights[x as usize])
    }

    pub fn on_stack(&self, piece: &FallingPiece) -> bool {
        piece.cells().iter().any(|&(x, y)| self.occupied(x, y - 1))
    }

    /// Does all logic associated with locking a piece.
    ///
    /// Clears lines, detects clear kind, calculates garbage, maintains combo and back-to-back
    /// state, detects perfect clears, detects lockout.
    pub fn lock_piece(&mut self, piece: FallingPiece) -> LockResult {
        let mut locked_out = true;
        for &(x, y) in &piece.cells() {
            self.cells[y as usize].set(x as usize, piece.kind.0.color());
            if self.column_heights[x as usize] < y + 1 {
                self.column_heights[x as usize] = y + 1;
            }
            if y < 20 {
                locked_out = false;
            }
        }
        let cleared = self.remove_cleared_lines();

        let placement_kind = PlacementKind::get(cleared.len(), piece.tspin);

        let mut garbage_sent = placement_kind.garbage();

        let mut did_b2b = false;
        if placement_kind.is_clear() {
            #[cfg(feature = "tetrio_garbage")]
            if placement_kind.is_hard() {
                if self.b2b_bonus > 0 {
                    did_b2b = true;
                }
                self.b2b_bonus += 1;
            } else {
                self.b2b_bonus = 0;
            }

            #[cfg(not(feature = "tetrio_garbage"))]
            if placement_kind.is_hard() {
                if self.b2b_bonus {
                    garbage_sent += 1;
                    did_b2b = true;
                }
                self.b2b_bonus = true;
            } else {
                self.b2b_bonus = false;
            }

            #[cfg(feature = "tetrio_garbage")]
                {
                    // garbage here is an f32 to make the calculation as accurate to the
                    // tetrio javascript code as possible
                    const B2B_BONUS_LOG: f32 = 0.8;
                    const COMBO_MINIFIER_LOG: f32 = 1.25;

                    let mut garbage = garbage_sent as f32;
                    if did_b2b {
                        let log = ((self.b2b_bonus as f32 - 1.0) * B2B_BONUS_LOG).ln_1p();

                        garbage += (1.0 + log).floor() + if self.b2b_bonus == 2 { 0.0 } else { (1.0 + log.fract()) / 3.0 };
                    }

                    garbage *= 1.0 + 0.25 * self.combo as f32;
                    garbage = (self.combo as f32 * COMBO_MINIFIER_LOG).ln_1p().max(garbage);
                    garbage_sent = garbage as u32;
                }

            #[cfg(not(feature = "tetrio_garbage"))]
                {
                    garbage_sent += COMBO_GARBAGE[(self.combo as usize).min(COMBO_GARBAGE.len() - 1)];
                }

            self.combo += 1;
        } else {
            self.combo = 0;
        }

        let perfect_clear = self.column_heights == [0; 10];
        #[cfg(feature = "tetrio_garbage")]
        if perfect_clear {
            garbage_sent += 10;
        }
        #[cfg(not(feature = "tetrio_garbage"))]
        if perfect_clear {
            garbage_sent = 10;
        }

        let l = LockResult {
            placement_kind,
            garbage_sent,
            perfect_clear,
            locked_out,
            combo: if self.combo == 0 {
                None
            } else {
                Some(self.combo - 1)
            },
            b2b: did_b2b,
            cleared_lines: cleared,
        };

        l
    }

    /// Holds the passed piece, returning the previous hold piece.
    ///
    /// If there is a piece in hold, it is returned.
    pub fn hold(&mut self, piece: Piece) -> Option<Piece> {
        let hold = self.hold_piece;
        self.hold_piece = Some(piece);
        hold
    }

    pub fn next_queue<'a>(&'a self) -> impl DoubleEndedIterator<Item=Piece> + 'a {
        self.next_pieces.iter().copied()
    }

    /// Returns the piece that should be spawned, or None if the queue is empty.
    pub fn advance_queue(&mut self) -> Option<Piece> {
        self.next_pieces.pop_front()
    }

    pub fn column_heights(&self) -> &[i32; 10] {
        &self.column_heights
    }

    pub fn add_garbage(&mut self, col: usize) -> bool {
        let mut row = *R::EMPTY;
        for x in 0..10 {
            if x == col {
                if self.column_heights[x] != 0 {
                    self.column_heights[x] += 1;
                }
            } else {
                row.set(x, CellColor::Garbage);
                self.column_heights[x] += 1;
            }
        }
        let dead = self.cells.pop().map_or(false, |r| !r.is_empty());
        self.cells.insert(0, row);
        dead
    }

    pub fn to_compressed(&self) -> Board {
        Board {
            cells: self
                .cells
                .iter()
                .map(|r| {
                    let mut row = 0;
                    for x in 0..10 {
                        row.set(x, r.cell_color(x));
                    }
                    row
                })
                .collect(),
            b2b_bonus: self.b2b_bonus,
            combo: self.combo,
            column_heights: self.column_heights,
            next_pieces: self.next_pieces.clone(),
            hold_piece: self.hold_piece,
            bag: self.bag,
        }
    }

    pub fn set_field(&mut self, field: [[bool; 10]; 40]) {
        self.cells.clear();
        self.column_heights = [0; 10];
        for y in 0..40 {
            let mut r = *R::EMPTY;
            for x in 0..10 {
                if field[y][x] {
                    r.set(x, CellColor::Garbage);
                    self.column_heights[x] = y as i32 + 1;
                }
            }
            self.cells.push(r)
        }
    }

    pub fn get_field(&self) -> [[bool; 10]; 40] {
        let mut field = [[false; 10]; 40];
        for y in 0..40 {
            for x in 0..10 {
                field[y][x] = self.occupied(x as i32, y as i32)
            }
        }
        field
    }

    pub fn next_bag(&self) -> EnumSet<Piece> {
        let mut bag = self.bag;
        for p in self.next_queue().rev() {
            if bag == EnumSet::all() {
                bag = EnumSet::empty();
            }
            bag.insert(p);
        }
        bag
    }
}

impl Row for u16 {
    fn set(&mut self, x: usize, color: CellColor) {
        if color == CellColor::Empty {
            *self &= !(1 << x);
        } else {
            *self |= 1 << x;
        }
    }

    #[inline]
    fn get(&self, x: usize) -> bool {
        *self & (1 << x) != 0
    }

    fn is_full(&self) -> bool {
        self == Self::SOLID
    }

    fn is_empty(&self) -> bool {
        self == Self::EMPTY
    }

    fn cell_color(&self, x: usize) -> CellColor {
        if self.get(x) {
            CellColor::Garbage
        } else {
            CellColor::Empty
        }
    }

    const SOLID: &'static u16 = &0b11111_11111;
    const EMPTY: &'static u16 = &0;
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

    fn is_empty(&self) -> bool {
        self.0.iter().all(|&c| c == CellColor::Empty)
    }

    const SOLID: &'static Self = &ColoredRow([CellColor::Unclearable; 10]);
    const EMPTY: &'static Self = &ColoredRow([CellColor::Empty; 10]);
}
