use libtetris::{ FallingPiece, Piece, Board };
use enumset::EnumSet;
use serde::{ Serialize, Deserialize };
use std::collections::HashMap;

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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PositionData {
    values: HashMap<Sequence, f64>,
    moves: Vec<Move>
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct Move {
    location: FallingPiece,
    value: Option<f64>
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

    pub fn value_of_position(&self, pos: Position) -> f64 {
        let mut value = 0.0;
        let mut count = 0;
        for (next, bag) in pos.next_possibilities() {
            count += 1;
            value += self.value_of_raw(pos, next, &[], bag);
        }
        value / count as f64
    }

    pub fn value_of(&self, state: &Board) -> f64 {
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

    fn value_of_raw(
        &self, pos: Position, next: EnumSet<Piece>, queue: &[Piece], bag: EnumSet<Piece>
    ) -> f64 {
        let values = match self.0.get(&pos) {
            Some(data) => &data.values,
            None => return 0.0
        };
        let possibilities = possible_sequences(
            queue.iter().copied().take(NEXT_PIECES).collect(), bag
        );
        let count = possibilities.len();
        possibilities.into_iter()
            .map(|(queue, _)| values.get(&Sequence { next, queue }).unwrap_or(&0.0))
            .sum::<f64>() / count as f64
    }

    fn update_value(&mut self, pos: Position) -> bool {
        let mut any_updated = false;
        for (next, bag) in pos.next_possibilities() {
            for (queue, qbag) in possible_sequences(vec![], bag) {
                let this = self.0.get(&pos).unwrap();
                let mut best = 0.0f64;
                for &mv in &this.moves {
                    if !next.contains(mv.location.kind.0) {
                        continue;
                    }
                    best = best.max(if let Some(v) = mv.value {
                        v
                    } else if next.len() == 1 {
                        self.value_of_raw(
                            pos.advance(mv.location),
                            next | queue[0],
                            &queue[1..],
                            qbag
                        )
                    } else {
                        self.value_of_raw(
                            pos.advance(mv.location),
                            next - mv.location.kind.0 | queue[0],
                            &queue[1..],
                            qbag
                        )
                    });
                }
                if best != 0.0 {
                    let old = self.0.get_mut(&pos).unwrap().values
                        .insert(Sequence { next, queue }, best);
                    if old != Some(best) {
                        any_updated = true;
                    }
                }
            }
        }
        any_updated
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
    }

    pub fn add_move(
        &mut self, position: impl Into<Position>, mv: FallingPiece, value: Option<f64>
    ) -> Position {
        let position = position.into();
        let moves = &mut self.0.entry(position).or_default().moves;
        if !moves.iter().any(|m| m.location == mv) {
            moves.push(Move {
                location: mv,
                value
            });
        }
        let next = position.advance(mv);
        self.0.entry(next).or_default();
        next
    }

    pub fn dump(&self) {
        fn name(pos: Position) -> String {
            let mut s = String::new();
            for &r in &pos.rows {
                s.push_str(&format!("{},", r));
            }
            for p in pos.bag {
                s.push(p.to_char());
            }
            if let Some(p) = pos.extra {
                s.push(p.to_char());
            }
            s
        }
        for (&pos, data) in &self.0 {
            use std::io::Write;
            let mut f = std::fs::File::create(format!("book/{}.html", name(pos))).unwrap();
            write!(f, r"
                <DOCTYPE html>
                <html>
                <head>
                    <style>
                        td {{
                            width: 16px;
                            height: 16px;
                            border: 1px solid black;
                        }}
                        table {{
                            border-collapse: collapse;
                        }}
                    </style>
                </head>
                <body>"
            ).unwrap();
            write!(f, "<p>Value: {:.5}", self.value_of_position(pos)).unwrap();
            write!(f, "<p>Bag: ").unwrap();
            for p in pos.bag.iter().chain(pos.extra.iter().copied()) {
                write!(f, "{}", p.to_char()).unwrap();
            }
            for mv in &data.moves {
                let cells = mv.location.cells();
                let target = pos.advance(mv.location);
                write!(f, "<div><a href='{}.html'>", name(target)).unwrap();
                match mv.value {
                    Some(v) => write!(f, "V={}", v),
                    None => write!(f, "E(V)={:.5}", self.value_of_position(target))
                }.unwrap();
                write!(f, "<table>").unwrap();
                for y in (0..6).rev() {
                    write!(f, "<tr>").unwrap();
                    for x in 0..10 {
                        write!(
                            f, "<td style='background-color: {}'></td>",
                            if pos.rows[y] & 1<<x != 0 {
                                "gray"
                            } else if cells.contains(&(x, y as i32)) {
                                match mv.location.kind.0 {
                                    Piece::I => "cyan",
                                    Piece::J => "blue",
                                    Piece::L => "orange",
                                    Piece::Z => "red",
                                    Piece::S => "green",
                                    Piece::T => "purple",
                                    Piece::O => "yellow"
                                }
                            } else {
                                "black"
                            }
                        ).unwrap();
                    }
                    write!(f, "</tr>").unwrap();
                }
                write!(f, "</table></a></div>").unwrap();
            }
            write!(f, "</body></html>").unwrap();
        }
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
    pub fn advance(&self, mv: FallingPiece) -> Position {
        let mut field = [[false; 10]; 40];
        for y in 0..10 {
            for x in 0..10 {
                field[y][x] = self.rows[y] & 1<<x != 0;
            }
        }
        let mut board = Board::new_with_state(field, self.bag, self.extra, false, 0);
        board.lock_piece(mv);
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
        position
    }

    fn next_possibilities(&self) -> Vec<(EnumSet<Piece>, EnumSet<Piece>)> {
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
