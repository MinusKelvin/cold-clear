use libtetris::{ FallingPiece, Piece, Board };
use enumset::EnumSet;
use serde::{ Serialize, Deserialize };
use std::collections::{ HashMap, HashSet };

const NEXT_PIECES: usize = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Book(HashMap<Position, PositionData>);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default, Serialize, Deserialize)]
pub struct Position {
    rows: [u16; 10],
    /// invariant: either this set has 2 elements in it, or the sole element is also the extra.
    bag: EnumSet<Piece>,
    /// invariant: if this is `Some`, the piece is also in the bag.
    extra: Option<Piece>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PositionData {
    values: HashMap<Sequence, (MoveValue, Vec<FallingPiece>)>,
    moves: Vec<Move>,
    dirty: bool
}

impl Default for PositionData {
    fn default() -> Self {
        PositionData {
            values: HashMap::new(),
            moves: vec![],
            dirty: true
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Move {
    pub location: FallingPiece,
    pub value: Option<f64>
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
struct Sequence {
    // this represents what can be placed with current or hold. if this has a single element,
    // that means that the current piece and the hold piece are the same.
    next: EnumSet<Piece>,
    queue: [Piece; NEXT_PIECES],
}

impl Book {
    pub fn new() -> Self {
        Book(HashMap::new())
    }

    pub fn load_from(src: impl std::io::Read) -> bincode::Result<Self> {
        bincode::deserialize_from(src)
    }

    pub fn save_to(&self, dst: impl std::io::Write) -> bincode::Result<()> {
        bincode::serialize_into(dst, self)
    }

    pub fn suggest_move(&self, state: &Board) -> Vec<FallingPiece> {
        let position = state.into();
        let mut next = EnumSet::empty();
        let mut q = state.next_queue();
        next.insert(q.next().unwrap());
        if let Some(p) = state.hold_piece {
            next.insert(p);
        } else {
            next.insert(q.next().unwrap());
        }
        self.suggest_move_raw(position, next, &q.collect::<Vec<_>>(), state.bag)
    }

    pub fn suggest_move_raw(
        &self, pos: Position, next: EnumSet<Piece>, queue: &[Piece], bag: EnumSet<Piece>
    ) -> Vec<FallingPiece> {
        let values = match self.0.get(&pos) {
            Some(data) => &data.values,
            None => return Default::default()
        };
        let possibilities = possible_sequences(
            queue.iter().copied().take(NEXT_PIECES).collect(), bag
        );
        let mut move_values = HashMap::new();
        let v = 1.0 / possibilities.len() as f64;
        let mut moves = vec![];
        for (queue, _) in possibilities {
            if let Some((_, best)) = values.get(&Sequence { next, queue }) {
                for &mv in best {
                    *move_values.entry(mv).or_insert(0.0) += v;
                    moves.push(mv);
                }
            }
        }
        moves.sort_by(|a, b| move_values.get(a).unwrap()
            .partial_cmp(move_values.get(b).unwrap())
            .unwrap()
        );
        moves
    }

    pub fn value_of_position(&self, pos: Position) -> MoveValue {
        pos.next_possibilities().into_iter()
            .map(|(next, bag)| self.value_of_raw(pos, next, &[], bag))
            .sum()
    }

    pub fn value_of(&self, state: &Board) -> MoveValue {
        let position = state.into();
        let mut next = EnumSet::empty();
        let mut q = state.next_queue();
        next.insert(q.next().unwrap());
        if let Some(p) = state.hold_piece {
            next.insert(p);
        } else {
            next.insert(q.next().unwrap());
        }
        self.value_of_raw(position, next, &q.collect::<Vec<_>>(), state.bag)
    }

    pub fn value_of_raw(
        &self, pos: Position, next: EnumSet<Piece>, queue: &[Piece], bag: EnumSet<Piece>
    ) -> MoveValue {
        let values = match self.0.get(&pos) {
            Some(data) => &data.values,
            None => return Default::default()
        };
        let possibilities = possible_sequences(
            queue.iter().copied().take(NEXT_PIECES).collect(), bag
        );
        possibilities.into_iter()
            .map(|(queue, _)| values.get(&Sequence { next, queue })
                .map(|&(v, _)| v)
                .unwrap_or_default())
            .sum()
    }

    fn update_value(&mut self, pos: Position) -> bool {
        let children_dirty = self.0.get(&pos).unwrap().moves.iter()
            .any(|m| self.0.get(&pos.advance(m.location).0).unwrap().dirty);
        if !self.0.get(&pos).unwrap().dirty && !children_dirty {
            return false;
        }
        self.0.get_mut(&pos).unwrap().dirty = false;
        for (next, bag) in pos.next_possibilities() {
            for (queue, qbag) in possible_sequences(vec![], bag) {
                let this = self.0.get(&pos).unwrap();
                let mut best = MoveValue::default();
                let mut best_moves = vec![];
                for &mv in &this.moves {
                    if !next.contains(mv.location.kind.0) {
                        continue;
                    }
                    let value = if let Some(value) = mv.value {
                        MoveValue {
                            long_moves: 0.0,
                            value
                        }
                    } else {
                        let (pos, long_moves) = pos.advance(mv.location);
                        let mut v = if next.len() == 1 {
                            self.value_of_raw(pos, next | queue[0], &queue[1..], qbag)
                        } else {
                            self.value_of_raw(
                                pos, next - mv.location.kind.0 | queue[0], &queue[1..], qbag
                            )
                        };
                        v.long_moves += long_moves;
                        v
                    };
                    if value > best {
                        best = value;
                        best_moves.clear();
                        best_moves.push(mv.location);
                    } else if value == best {
                        best_moves.push(mv.location);
                    }
                }
                if best != MoveValue::default() {
                    let this = self.0.get_mut(&pos).unwrap();
                    let old = this.values.insert(Sequence { next, queue }, (best, best_moves));
                    this.dirty |= old.map(|v| v.0) != Some(best);
                }
            }
        }
        self.0.get(&pos).unwrap().dirty
    }

    pub fn recalculate_graph(&mut self) {
        let positions: Vec<_> = self.0.keys().copied().collect();
        for _ in 0..positions.len() {
            let mut any_updated = false;
            for &pos in &positions {
                any_updated |= self.update_value(pos);
            }
            if !any_updated {
                break;
            }
        }
        self.0.retain(|_, v| !v.values.is_empty());
        let positions: HashSet<Position> = self.0.keys().copied().collect();
        for &pos in &positions {
            self.0.get_mut(&pos).unwrap().moves.retain(
                |m| m.value.is_some() || positions.contains(&pos.advance(m.location).0)
            );
        }
    }

    pub fn add_move(
        &mut self, position: impl Into<Position>, mv: FallingPiece, value: Option<f64>
    ) -> Position {
        let position = position.into();
        let moves = &mut self.0.entry(position).or_default().moves;
        match moves.iter_mut().find(|m| m.location.same_location(&mv)) {
            Some(mv) => if mv.value < value {
                mv.value = value;
            }
            None => moves.push(Move {
                location: mv,
                value
            })
        }
        let next = position.advance(mv).0;
        self.0.entry(next).or_default();
        next
    }

    pub fn moves(&self, pos: Position) -> &[Move] {
        self.0.get(&pos)
            .map(|data| &*data.moves)
            .unwrap_or(&[])
    }

    pub fn positions<'a>(&'a self) -> impl Iterator<Item=Position> + 'a {
        self.0.keys().copied()
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

impl Position {
    pub fn advance(&self, mv: FallingPiece) -> (Position, f64) {
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
        (position, soft_drop as u8 as f64 + clear as u8 as f64)
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

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MoveValue {
    pub value: f64,
    pub long_moves: f64
}

impl MoveValue {
    pub fn max(self, other: Self) -> Self {
        if self < other {
            other
        } else {
            self
        }
    }
}

impl std::cmp::PartialOrd for MoveValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.value.partial_cmp(&other.value) {
            Some(std::cmp::Ordering::Equal) => other.long_moves.partial_cmp(&self.long_moves),
            order => order
        }
    }
}

impl std::iter::Sum for MoveValue {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut this = MoveValue::default();
        let mut long_count = 0;
        let mut value_count = 0;
        for v in iter {
            value_count += 1;
            this.value += v.value;
            if v.value != 0.0 {
                long_count += 1;
                this.long_moves += v.long_moves;
            }
        }
        if long_count != 0 {
            this.long_moves /= long_count as f64;
        }
        if value_count != 0 {
            this.value /= value_count as f64;
        }
        this
    }
}