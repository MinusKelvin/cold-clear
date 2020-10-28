use crate::*;
use std::collections::{ HashMap, HashSet, VecDeque };

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BookBuilder {
    data: HashMap<Position, PositionData>,
    dirty_positions: HashSet<Position>,
    dirty_queue: VecDeque<Position>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PositionData {
    values: HashMap<Sequence, (MoveValue, CompactPiece)>,
    moves: Vec<Move>,
    backrefs: Vec<Position>
}

impl Default for PositionData {
    fn default() -> Self {
        PositionData {
            values: HashMap::new(),
            moves: vec![],
            backrefs: vec![]
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Move {
    location: CompactPiece,
    value: OptionNanF32
}

impl Move {
    pub fn location(&self) -> FallingPiece {
        self.location.into()
    }

    pub fn value(&self) -> Option<f32> {
        self.value.into()
    }
}

impl BookBuilder {
    pub fn new() -> Self {
        BookBuilder {
            data: HashMap::new(),
            dirty_positions: HashSet::new(),
            dirty_queue: VecDeque::new()
        }
    }

    pub fn suggest_move(&self, state: &Board) -> Option<FallingPiece> {
        let position = state.into();
        let mut next = EnumSet::empty();
        let mut q = state.next_queue();
        next.insert(q.next().unwrap());
        if let Some(p) = state.hold_piece {
            next.insert(p);
        } else {
            next.insert(q.next().unwrap());
        }
        self.suggest_move_raw(position, next, &q.collect::<Vec<_>>())
    }

    pub fn suggest_move_raw(
        &self, pos: Position, next: EnumSet<Piece>, queue: &[Piece]
    ) -> Option<FallingPiece> {
        let values = &self.data.get(&pos)?.values;
        let queue = queue.iter().copied().take(NEXT_PIECES)
            .collect::<arrayvec::ArrayVec<[_; NEXT_PIECES]>>()
            .into_inner().ok()?;
        values.get(&Sequence { next, queue }).map(|&(_, mv)| mv.into())
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
        let values = match self.data.get(&pos) {
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

    fn update_value(&mut self, pos: Position) {
        let mut anything_changed = false;
        for (next, bag) in pos.next_possibilities() {
            for (queue, qbag) in possible_sequences(vec![], bag) {
                let this = self.data.get(&pos).unwrap();
                let mut best = MoveValue::default();
                let mut best_move = None;
                for &mv in &this.moves {
                    let current_mv = mv.location();
                    if !next.contains(current_mv.kind.0) {
                        continue;
                    }
                    let (pos, long_moves) = pos.advance(mv.location.into());
                    let mut value = if let Some(value) = mv.value.into() {
                        MoveValue {
                            long_moves: 0.0,
                            value
                        }
                    } else if next.len() == 1 {
                        self.value_of_raw(pos, next | queue[0], &queue[1..], qbag)
                    } else {
                        self.value_of_raw(
                            pos, next - current_mv.kind.0 | queue[0], &queue[1..], qbag
                        )
                    };
                    value.long_moves += long_moves;
                    if value > best {
                        best = value;
                        best_move = Some(mv.location);
                    } else if value == best && best != MoveValue::default() {
                        let best_mv: FallingPiece = best_move.unwrap().into();
                        let ord = current_mv.x.cmp(&best_mv.x)
                            .then(current_mv.y.cmp(&best_mv.y))
                            .then((current_mv.kind.0 as usize).cmp(&(best_mv.kind.0 as usize)))
                            .then((current_mv.kind.1 as usize).cmp(&(best_mv.kind.1 as usize)));
                        if ord == std::cmp::Ordering::Greater {
                            best_move = Some(mv.location);
                        }
                    }
                }
                if let Some(best_move) = best_move {
                    let this = self.data.get_mut(&pos).unwrap();
                    let old = this.values.insert(Sequence { next, queue }, (best, best_move));
                    anything_changed |= old.map(|v| v.0) != Some(best);
                }
            }
        }
        if anything_changed {
            for &parent in &self.data.get(&pos).unwrap().backrefs {
                if self.dirty_positions.insert(parent) {
                    self.dirty_queue.push_back(parent);
                }
            }
        }
    }

    pub fn recalculate_graph(&mut self) {
        while let Some(to_update) = self.dirty_queue.pop_front() {
            self.dirty_positions.remove(&to_update);
            self.update_value(to_update);
        }
    }

    pub fn add_move(
        &mut self, position: impl Into<Position>, mv: FallingPiece, value: Option<f32>
    ) {
        let position = position.into();
        let moves = &mut self.data.entry(position).or_default().moves;
        let mut add_backref = false;
        let mut remove_backref = false;
        match moves.iter_mut().find(|m| m.location().same_location(&mv)) {
            Some(mv) => if mv.value() < value {
                remove_backref = mv.value().is_none() && value.is_some();
                mv.value = value.into();
            }
            None => {
                add_backref = value.is_none();
                moves.push(Move {
                    location: mv.into(),
                    value: value.into()
                });
            }
        }
        if add_backref {
            self.data.entry(position.advance(mv).0).or_default().backrefs.push(position);
        }
        if remove_backref {
            self.data.entry(position.advance(mv).0).and_modify(
                |v| v.backrefs.retain(|&p| p != position)
            );
        }
        if value.is_some() {
            if self.dirty_positions.insert(position) {
                self.dirty_queue.push_back(position);
            }
        }
    }

    pub fn moves(&self, pos: Position) -> &[Move] {
        self.data.get(&pos)
            .map(|data| &*data.moves)
            .unwrap_or(&[])
    }

    pub fn positions<'a>(&'a self) -> impl Iterator<Item=Position> + 'a {
        self.data.keys().copied()
    }

    pub fn compile(mut self, roots: &[Position]) -> Book {
        let mut book = HashMap::new();
        let mut to_compile = roots.to_vec();
        while let Some(pos) = to_compile.pop() {
            book.entry(pos).or_insert_with(|| {
                let moves = self.build_position(&pos);
                for &(_, m) in &moves {
                    if let Some(p) = m {
                        to_compile.push(pos.advance(p.into()).0);
                    }
                }
                moves
            });
        }
        Book(book)
    }

    fn build_position(&mut self, pos: &Position) -> Vec<(Sequence, Option<CompactPiece>)> {
        let mut sequences = vec![];
        for (next, bag) in pos.next_possibilities() {
            for (queue, _) in possible_sequences(vec![], bag) {
                let seq = Sequence { next, queue };
                let mv = self.suggest_move_raw(*pos, next, &queue);
                sequences.push((seq, mv.map(Into::into)));
            }
        }
        self.data.remove(pos);
        sequences.sort_by_key(|&(s, _)| s);
        sequences.dedup_by_key(|&mut (_, m)| m);
        sequences.shrink_to_fit();
        sequences
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MoveValue {
    pub value: f32,
    pub long_moves: f32
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
            this.long_moves /= long_count as f32;
        }
        if value_count != 0 {
            this.value /= value_count as f32;
        }
        this
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct OptionNanF32(f32);

impl From<Option<f32>> for OptionNanF32 {
    fn from(v: Option<f32>) -> Self {
        match v {
            Some(v) => Self(v),
            None => Self(std::f32::NAN)
        }
    }
}

impl From<OptionNanF32> for Option<f32> {
    fn from(v: OptionNanF32) -> Self {
        if v.0.is_nan() {
            None
        } else {
            Some(v.0)
        }
    }
}