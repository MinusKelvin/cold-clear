use libtetris::{ FallingPiece, Piece, RotationState, Board };
use enumset::EnumSet;
use serde::{ Serialize, Deserialize };
use std::collections::HashMap;
use std::io::prelude::*;

const NEXT_PIECES: usize = 4;

#[cfg(feature = "builder")]
mod builder;
#[cfg(feature = "builder")]
pub use builder::BookBuilder;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Book(HashMap<Position, Vec<(Sequence, Option<CompactPiece>)>>);

impl Book {
    pub fn load(from: impl Read) -> Result<Self, bincode::Error> {
        bincode::deserialize_from(xz2::read::XzDecoder::new(from))
    }

    pub fn save(&self, to: impl Write) -> Result<(), bincode::Error> {
        let mut to = xz2::write::XzEncoder::new(to, 9);
        bincode::serialize_into(&mut to, self)?;
        to.finish()?;
        Ok(())
    }

    pub fn suggest_move(&self, state: &Board) -> Option<FallingPiece> {
        let position = state.into();
        let mut next = EnumSet::empty();
        let mut q = state.next_queue();
        next.insert(q.next()?);
        if let Some(p) = state.hold_piece {
            next.insert(p);
        } else {
            next.insert(q.next()?);
        }
        let q = [q.next()?, q.next()?, q.next()?, q.next()?];
        self.suggest_move_raw(position, next, q)
    }

    fn suggest_move_raw(
        &self, pos: Position, next: EnumSet<Piece>, queue: [Piece; NEXT_PIECES]
    ) -> Option<FallingPiece> {
        let to_find = Sequence { next, queue };
        let moves = self.0.get(&pos)?;
        match moves.binary_search_by_key(&to_find, |&(s,_)| s) {
            Result::Ok(i) => moves[i].1.map(Into::into),
            Result::Err(i) => moves[i-1].1.map(Into::into)
        }
    }

    pub fn merge(&mut self, other: Book) {
        for (pos, data) in other.0 {
            self.0.entry(pos).or_insert(data);
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default, Serialize, Deserialize)]
pub struct Position {
    rows: [u16; 10],
    /// invariant: either this set has >=2 elements in it, or the sole element is also the extra.
    bag: EnumSet<Piece>,
    /// invariant: if this is `Some`, the piece is also in the bag.
    extra: Option<Piece>
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
struct Sequence {
    // this represents what can be placed with current or hold. if this has a single element,
    // that means that the current piece and the hold piece are the same.
    next: EnumSet<Piece>,
    queue: [Piece; NEXT_PIECES],
}

impl Position {
    pub fn advance(&self, mv: FallingPiece) -> (Position, f32) {
        let mut field = [[false; 10]; 40];
        for y in 0..10 {
            for x in 0..10 {
                field[y][x] = self.rows[y] & 1<<x != 0;
            }
        }
        let mut board = Board::new_with_state(field, self.bag, self.extra, false, 0);
        let soft_drop = !board.above_stack(&mv);
        let clear = board.lock_piece(mv).placement_kind.is_clear();
        let mut position = *self;
        for y in 0..10 {
            position.rows[y] = *board.get_row(y as i32);
        }
        if self.extra == Some(mv.kind.0) {
            position.extra = None;
            if position.bag.len() == 1 {
                position.extra = position.bag.iter().next();
                position.bag = EnumSet::all();
            }
        } else {
            position.bag.remove(mv.kind.0);
            if position.bag.len() == 1 && position.extra.is_none() {
                position.extra = position.bag.iter().next();
                position.bag = EnumSet::all();
            }
        }
        (position, soft_drop as u8 as f32 + clear as u8 as f32)
    }

    pub fn next_possibilities(&self) -> Vec<(EnumSet<Piece>, EnumSet<Piece>)> {
        let mut next_possibilities = vec![];
        match self.extra {
            Some(p) => for other in self.bag {
                next_possibilities.push((p | other, refill_if_empty(self.bag - other)));
            }
            None => {
                let bag: Vec<_> = self.bag.iter().collect();
                for i in 0..bag.len() {
                    for j in i+1..bag.len() {
                        next_possibilities.push(
                            (bag[i] | bag[j], refill_if_empty(self.bag - bag[i] - bag[j]))
                        );
                    }
                }
            }
        }
        next_possibilities
    }

    pub fn bag(&self) -> EnumSet<Piece> {
        self.bag
    }

    pub fn extra(&self) -> Option<Piece> {
        self.extra
    }

    pub fn rows(&self) -> &[u16] {
        &self.rows
    }
}

impl From<&Board> for Position {
    fn from(v: &Board) -> Position {
        let mut this = Position {
            rows: [0; 10],
            bag: v.next_bag(),
            extra: None
        };
        if let Some(hold) = v.hold_piece {
            if this.bag.contains(hold) {
                this.extra = Some(hold);
            } else {
                this.bag.insert(hold);
            }
        }
        if this.bag.len() == 1 && this.extra.is_none() {
            this.extra = this.bag.iter().next();
            this.bag = EnumSet::all();
        }
        for y in 0..10 {
            this.rows[y] = *v.get_row(y as i32);
        }
        this
    }
}

impl From<Board> for Position {
    fn from(v: Board) -> Position {
        (&v).into()
    }
}

fn refill_if_empty<T: enumset::EnumSetType>(bag: EnumSet<T>) -> EnumSet<T> {
    if bag.is_empty() {
        EnumSet::all()
    } else {
        bag
    }
}

pub fn possible_sequences(
    mut q: Vec<Piece>, bag: EnumSet<Piece>
) -> Vec<([Piece; NEXT_PIECES], EnumSet<Piece>)> {
    fn solve(
        q: &mut Vec<Piece>,
        bag: EnumSet<Piece>,
        out: &mut Vec<([Piece; NEXT_PIECES], EnumSet<Piece>)>
    ) {
        use std::convert::TryFrom;
        match <&[_; NEXT_PIECES]>::try_from(&**q) {
            Ok(&q) => out.push((q, bag)),
            Err(_) => for p in bag {
                let new_bag = refill_if_empty(bag - p);
                q.push(p);
                solve(q, new_bag, out);
                q.pop();
            }
        }
    }

    let mut result = vec![];
    solve(&mut q, bag, &mut result);
    result
}

impl Ord for Sequence {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let mut i = self.next.iter();
        let p1 = i.next().unwrap();
        let p2 = i.next().unwrap_or(p1);
        let mut q1 = vec![PieceOrd(p1), PieceOrd(p2)];
        q1.extend(self.queue.iter().map(|&p| PieceOrd(p)));

        let mut i = other.next.iter();
        let p1 = i.next().unwrap();
        let p2 = i.next().unwrap_or(p1);
        let mut q2 = vec![PieceOrd(p1), PieceOrd(p2)];
        q2.extend(other.queue.iter().map(|&p| PieceOrd(p)));

        q1.cmp(&q2)
    }
}

impl PartialOrd for Sequence {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct PieceOrd(Piece);

impl Ord for PieceOrd {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.0 as usize).cmp(&(other.0 as usize))
    }
}

impl PartialOrd for PieceOrd {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
struct CompactPiece(std::num::NonZeroU16);

impl std::fmt::Debug for CompactPiece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", FallingPiece::from(*self))
    }
}

impl From<FallingPiece> for CompactPiece {
    fn from(v: FallingPiece) -> Self {
        let p = match v.kind.0 {
            Piece::I => 1,
            Piece::O => 2,
            Piece::T => 3,
            Piece::L => 4,
            Piece::J => 5,
            Piece::S => 6,
            Piece::Z => 7,
        };
        let r = match v.kind.1 {
            RotationState::North => 0,
            RotationState::South => 1,
            RotationState::East => 2,
            RotationState::West => 3
        };
        CompactPiece(std::num::NonZeroU16::new(
            p | r << 3 | (v.x as u16) << 5 | (v.y as u16) << 9
        ).unwrap())
    }
}

impl From<CompactPiece> for FallingPiece {
    fn from(v: CompactPiece) -> FallingPiece {
        FallingPiece {
            kind: libtetris::PieceState(
                match v.0.get() & 0b111 {
                    1 => Piece::I,
                    2 => Piece::O,
                    3 => Piece::T,
                    4 => Piece::L,
                    5 => Piece::J,
                    6 => Piece::S,
                    7 => Piece::Z,
                    _ => unreachable!(),
                },
                match v.0.get() >> 3 & 0b11 {
                    0 => RotationState::North,
                    1 => RotationState::South,
                    2 => RotationState::East,
                    3 => RotationState::West,
                    _ => unreachable!()
                }
            ),
            x: (v.0.get() >> 5 & 0b1111) as i32,
            y: (v.0.get() >> 9 & 0b111111) as i32,
            tspin: libtetris::TspinStatus::None
        }
    }
}
