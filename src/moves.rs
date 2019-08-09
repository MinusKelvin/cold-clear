use crate::tetris::{ BoardState, FallingPiece };
use std::collections::{ HashMap, HashSet, VecDeque, hash_map::Entry };

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Input {
    Left,
    Right,
    Cw,
    Ccw,
    SonicDrop,
    DasLeft,
    DasRight
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Move {
    pub inputs: Vec<Input>,
    pub location: FallingPiece
}

pub fn find_moves(board: &BoardState, spawned: FallingPiece, use_das: bool) -> Vec<Move> {
    let t = std::time::Instant::now();
    let mut locks = HashMap::new();
    let mut checked = HashSet::new();
    let mut check_queue = VecDeque::new();
    check_queue.push_back((vec![], spawned));

    while let Some((moves, position)) = check_queue.pop_front() {
        let mut change = position;
        if change.shift(board, -1) {
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::Left);
                check_queue.push_back((m, change));
            }
        }

        let mut change = position;
        if change.shift(board, 1) {
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::Right);
                check_queue.push_back((m, change));
            }
        }

        let mut change = position;
        if change.cw(board) {
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::Cw);
                check_queue.push_back((m, change));
            }
        }

        let mut change = position;
        if change.ccw(board) {
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::Ccw);
                check_queue.push_back((m, change));
            }
        }

        if use_das {
            let mut change = position;
            while change.shift(board, -1) {}
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::DasLeft);
                check_queue.push_back((m, change));
            }

            let mut change = position;
            while change.shift(board, 1) {}
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::DasRight);
                check_queue.push_back((m, change));
            }
        }

        let mut change = position;
        if change.sonic_drop(board) {
            if checked.insert(change) {
                let mut m = moves.clone();
                m.push(Input::SonicDrop);
                check_queue.push_back((m, change));
            }
        }

        let cells = change.cells();
        if cells.iter().all(|&(_, y)| y >= 20) {
            continue
        }
        match locks.entry(cells) {
            Entry::Vacant(entry) => {
                entry.insert(Move {
                    inputs: moves,
                    location: change
                });
            }
            Entry::Occupied(mut entry) => {
                let mv = entry.get_mut();
                let us_sdrops = moves.iter().filter(|&&v| v == Input::SonicDrop).count();
                let them_sdrops = mv.inputs.iter().filter(|&&v| v == Input::SonicDrop).count();
                if us_sdrops < them_sdrops ||
                        (us_sdrops == them_sdrops && moves.len() < mv.inputs.len()) {
                    *mv = Move {
                        inputs: moves,
                        location: change
                    };
                }
            }
        }
    }

    let v: Vec<_> = locks.into_iter().map(|(_, v)| v).collect();
    eprintln!("Found {} moves in {:?}", v.len(), t.elapsed());
    v
}

impl Input {
    pub fn to_char(self) -> char {
        match self {
            Input::Left => '<',
            Input::Right => '>',
            Input::Cw => 'r',
            Input::Ccw => 'l',
            Input::SonicDrop => 'v',
            Input::DasLeft => '[',
            Input::DasRight => ']'
        }
    }
}