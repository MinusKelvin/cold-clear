use std::collections::VecDeque;

use libtetris::{ Board, LockResult, Piece, FallingPiece };
use crate::moves::Move;

pub struct Tree {
    pub board: Board,
    pub raw_eval: crate::evaluation::Evaluation,
    pub evaluation: Option<i32>,
    // TODO: newtype this vec type
    children: Vec<(Move, LockResult, Tree)>,
    // TODO: switch to Option<Vec<(Move, LockResult, Tree)>>
    // After all, we don't use the board/evaluation/hold_case fields of this tree
    hold_case: Option<Box<Tree>>
}

enum Child<'a> {
    None,
    HoldCase(&'a Tree),
    Move(usize, &'a Move, &'a LockResult, &'a Tree)
}

enum ChildMut<'a> {
    None,
    HoldCase(&'a mut Tree),
    Move(usize, &'a mut Move, &'a mut LockResult, &'a mut Tree)
}

impl Tree {
    pub fn new(
        board: Board,
        lock: &LockResult,
        soft_dropped: bool,
        transient_weights: &crate::evaluation::BoardWeights,
        acc_weights: &crate::evaluation::PlacementWeights
    ) -> Self {
        use crate::evaluation::evaluate;
        let raw_eval = evaluate(lock, &board, transient_weights, acc_weights, soft_dropped);
        Tree {
            raw_eval, board,
            evaluation: Some(raw_eval.accumulated + raw_eval.transient),
            children: vec![],
            hold_case: None
        }
    }

    pub fn repropagate(&mut self, mut path: VecDeque<Option<Move>>) {
        match path.pop_front() {
            None => match self.best_child() {
                Child::HoldCase(t) => self.evaluation = t.evaluation,
                Child::Move(_, _, _, t) =>
                    self.evaluation = Some(t.evaluation.unwrap() + self.raw_eval.accumulated),
                Child::None => {}
            }
            Some(None) => {
                let hold = self.hold_case.as_mut().unwrap();
                hold.repropagate(path);
                match self.best_child() {
                    Child::HoldCase(t) => self.evaluation = t.evaluation,
                    Child::Move(_, _, _, t) =>
                        self.evaluation = Some(t.evaluation.unwrap() + self.raw_eval.accumulated),
                    Child::None => self.evaluation = None
                }
            }
            Some(Some(m)) => {
                for (mv, _, t) in &mut self.children {
                    if *mv == m {
                        t.repropagate(path);
                        break
                    }
                }
                match self.best_child() {
                    Child::HoldCase(t) => self.evaluation = t.evaluation,
                    Child::Move(_, _, _, t) =>
                        self.evaluation = Some(t.evaluation.unwrap() + self.raw_eval.accumulated),
                    Child::None => self.evaluation = None
                }
            }
        }
    }

    fn best_child(&self) -> Child {
        let mut best = if let Some(hold) = &self.hold_case {
            if hold.evaluation.is_some() {
                Child::HoldCase(hold)
            } else {
                Child::None
            }
        } else {
            Child::None
        };
        for (i, (mv, r, t)) in self.children.iter().enumerate() {
            if t.evaluation.is_none() {
                continue
            }
            match best {
                Child::None => best = Child::Move(i, mv, r, t),
                Child::HoldCase(b) => if t.evaluation > b.evaluation {
                    best = Child::Move(i, mv, r, t);
                }
                Child::Move(_, _, _, bt) => if t.evaluation > bt.evaluation {
                    best = Child::Move(i, mv, r, t);
                }
            }
        }
        best
    }

    fn best_child_mut(&mut self) -> ChildMut {
        let mut best = if let Some(hold) = &mut self.hold_case {
            if hold.evaluation.is_some() {
                ChildMut::HoldCase(hold)
            } else {
                ChildMut::None
            }
        } else {
            ChildMut::None
        };
        for (i, (mv, r, t)) in self.children.iter_mut().enumerate() {
            if t.evaluation.is_none() {
                continue
            }
            match best {
                ChildMut::None => best = ChildMut::Move(i, mv, r, t),
                ChildMut::HoldCase(b) => if t.evaluation > b.evaluation {
                    best = ChildMut::Move(i, mv, r, t);
                } else {
                    best = ChildMut::HoldCase(b);
                }
                ChildMut::Move(bi, bmv, br, bt) => if t.evaluation > bt.evaluation {
                    best = ChildMut::Move(i, mv, r, t);
                } else {
                    best = ChildMut::Move(bi, bmv, br, bt);
                }
            }
        }
        best
    }

    pub fn take_best_move(&mut self) -> Option<(bool, Move, LockResult, Tree)> {
        match self.best_child_mut() {
            ChildMut::HoldCase(_) => self.hold_case.as_mut()
                .and_then(|t| t.take_best_move())
                .map(|(_, mv, r, t)| (true, mv, r, t)),
            ChildMut::Move(i, _, _, _) => {
                let (mv, r, t) = self.children.remove(i);
                Some((false, mv, r, t))
            }
            ChildMut::None => None
        }
    }

    pub fn add_next_piece(&mut self, piece: Piece) {
        self.board.add_next_piece(piece);
        if let Some(hold) = &mut self.hold_case {
            hold.add_next_piece(piece);
        }
        for (_, _, t) in &mut self.children {
            t.add_next_piece(piece);
        }
    }

    pub fn depth(&self) -> u32 {
        match self.best_child() {
            Child::HoldCase(t) => t.depth(),
            Child::Move(_, _, _, t) => 1 + t.depth(),
            Child::None => 0
        }
    }

    pub fn branches(&self) -> Vec<(bool, usize)> {
        let mut branches = self.hold_case.as_ref().map_or(vec![], |hold_case| {
            let mut v = hold_case.branches();
            for (held, _) in &mut v {
                *held = true
            }
            v
        });
        for (i, (_, _, t)) in self.children.iter().enumerate() {
            if t.evaluation.is_some() {
                branches.push((false, i));
            }
        }
        branches
    }

    pub fn branch(&self, (hold, i): (bool, usize)) -> &(Move, LockResult, Tree) {
        &if hold {
            &**self.hold_case.as_ref().unwrap()
        } else {
            self
        }.children[i]
    }

    pub fn branch_mut(&mut self, (hold, i): (bool, usize)) -> &mut (Move, LockResult, Tree) {
        &mut if hold {
            &mut **self.hold_case.as_mut().unwrap()
        } else {
            self
        }.children[i]
    }

    pub fn extensions(&self, mode: crate::moves::MovementMode) -> Vec<(bool, Move)> {
        let mut extensions = vec![];
        match self.board.get_next_piece() {
            Ok(piece) => match FallingPiece::spawn(piece, &self.board) {
                Some(spawned) => {
                    for mv in crate::moves::find_moves(&self.board, spawned, mode) {
                        extensions.push((false, mv));
                    }
                    let hold = self.board.hold_piece()
                        .or(self.board.get_next_next_piece())
                        .and_then(|p|
                            if p == piece {
                                None
                            } else {
                                FallingPiece::spawn(p, &self.board)
                            }
                        );
                    if let Some(spawned) = hold {
                        for mv in crate::moves::find_moves(&self.board, spawned, mode) {
                            extensions.push((true, mv));
                        }
                    }
                }
                None => {}
            }
            Err(possiblilities) => {
                // TODO: Speculation is not yet implemented
            }
        }
        extensions
    }

    pub fn extend(&mut self, hold: bool, mv: Move, result: LockResult, subtree: Tree) {
        if hold {
            let b = &self.board;
            let raw_eval = self.raw_eval;
            self.hold_case.get_or_insert_with(|| Box::new(Tree {
                board: b.clone(),
                evaluation: None,
                raw_eval,
                hold_case: None,
                children: vec![]
            })).children.push((mv, result, subtree));
        } else {
            self.children.push((mv, result, subtree));
        }
    }
}