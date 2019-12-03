use rand::prelude::*;
use enum_map::EnumMap;
use arrayvec::ArrayVec;
use odds::vec::VecExt;
use libtetris::{ Board, LockResult, Piece, FallingPiece, PlacementKind };
use crate::moves::Placement;
use crate::evaluation::{ Evaluator, Evaluation };
use crate::Options;

pub struct Tree {
    pub board: Board,
    pub raw_eval: Evaluation,
    pub evaluation: i32,
    pub depth: usize,
    pub child_nodes: usize,
    kind: Option<TreeKind>
}

enum TreeKind {
    Known(Vec<Child>),
    Unknown(Speculation)
}

type Speculation = EnumMap<Piece, Option<Vec<Child>>>;

pub struct Child {
    pub hold: bool,
    pub mv: Placement,
    pub lock: LockResult,
    pub original_rank: usize,
    pub tree: Tree
}

impl Tree {
    pub fn starting(board: Board) -> Self {
        Tree {
            board,
            raw_eval: Evaluation { accumulated: 0, transient: 0 },
            evaluation: 0, depth: 0, child_nodes: 0, kind: None
        }
    }

    pub fn new(
        board: Board,
        lock: &LockResult,
        move_time: u32,
        piece: Piece,
        evaluator: &impl Evaluator
    ) -> Self {
        let raw_eval = evaluator.evaluate(lock, &board, move_time, piece);
        Tree {
            raw_eval, board,
            evaluation: raw_eval.accumulated + raw_eval.transient,
            depth: 0,
            child_nodes: 0,
            kind: None
        }
    }

    pub fn into_best_child(mut self) -> Result<Child, Tree> {
        match self.kind {
            None => Err(self),
            Some(tk) => {
                match tk.into_best_child() {
                    Ok(c) => Ok(c),
                    Err(tk) => {
                        self.kind = Some(tk);
                        Err(self)
                    }
                }
            }
        }
    }

    pub fn get_best_child(&self) -> Option<&Child> {
        self.kind.as_ref().and_then(|tk| tk.get_best_child())
    }

    pub fn get_plan(&self, into: &mut Vec<(Placement, LockResult)>) {
        if let Some(ref tk) = self.kind {
            tk.get_plan(into);
        }
    }

    pub fn get_moves_and_evaluations(&self) -> Vec<(FallingPiece, i32)> {
        if let Some(ref tk) = self.kind {
            tk.get_moves_and_evaluations()
        } else {
            vec![]
        }
    }

    /// Returns is_death
    pub fn add_next_piece(&mut self, piece: Piece, options: Options) -> bool {
        self.board.add_next_piece(piece);
        if let Some(ref mut k) = self.kind {
            if k.add_next_piece(piece, options) {
                true
            } else {
                self.evaluation = self.raw_eval.accumulated + k.evaluation();
                false
            }
        } else {
            false
        }
    }

    /// Does an iteration of MCTS. Returns true if only death is possible from this position.
    pub fn extend(
        &mut self, opts: Options, evaluator: &impl Evaluator
    ) -> bool {
        self.expand(opts, evaluator).is_death
    }

    fn expand(
        &mut self, opts: Options, evaluator: &impl Evaluator
    ) -> ExpandResult {
        match self.kind {
            // TODO: refactor the unexpanded case into TreeKind, and remove the board field
            Some(ref mut tk) => {
                let er = tk.expand(opts, evaluator);
                if !er.is_death {
                    // Update this node's information
                    self.evaluation = self.raw_eval.accumulated + tk.evaluation();
                    self.depth = self.depth.max(er.depth);
                    self.child_nodes += er.new_nodes;
                }
                er
            }
            None => {
                if self.board.get_next_piece().is_ok() {
                    if opts.use_hold && self.board.hold_piece().is_none() &&
                            self.board.get_next_next_piece().is_none() {
                        // Speculate - next piece is known, but hold piece isn't
                        self.speculate(opts, evaluator)
                    } else {
                        // Both next piece and hold piece are known
                        let children = new_children(
                            self.board.clone(), opts, evaluator
                        );

                        if children.is_empty() {
                            ExpandResult {
                                is_death: true,
                                depth: 0,
                                new_nodes: 0
                            }
                        } else {
                            self.depth = 1;
                            self.child_nodes = children.len();
                            let tk = TreeKind::Known(children);
                            self.evaluation = self.raw_eval.accumulated + tk.evaluation();
                            self.kind = Some(tk);
                            ExpandResult {
                                is_death: false,
                                depth: 1,
                                new_nodes: self.child_nodes
                            }
                        }
                    }
                } else {
                    // Speculate - hold should be known, but next piece isn't
                    if opts.use_hold {
                        assert!(
                            self.board.hold_piece().is_some(),
                            "Neither hold piece or next piece are known - what the heck happened?\n\
                            get_next_piece: {:?}", self.board.get_next_piece()
                        );
                    }
                    self.speculate(opts, evaluator)
                }
            }
        }
    }

    fn speculate(
        &mut self,
        opts: Options,
        evaluator: &impl Evaluator
    ) -> ExpandResult {
        if !opts.speculate {
            return ExpandResult {
                is_death: false,
                depth: 0,
                new_nodes: 0
            }
        }
        let possibilities = match self.board.get_next_piece() {
            Ok(_) => {
                let mut b = self.board.clone();
                b.advance_queue();
                b.get_next_piece().unwrap_err()
            }
            Err(possibilities) => possibilities
        };
        let mut speculation = EnumMap::new();
        for piece in possibilities.iter() {
            let mut board = self.board.clone();
            board.add_next_piece(piece);
            let children = new_children(
                board, opts, evaluator
            );
            self.child_nodes += children.len();
            speculation[piece] = Some(children);
        }

        if self.child_nodes == 0 {
            ExpandResult {
                is_death: true,
                depth: 0,
                new_nodes: 0
            }
        } else {
            let tk = TreeKind::Unknown(speculation);
            self.evaluation = self.raw_eval.accumulated + tk.evaluation();
            self.kind = Some(tk);
            self.depth = 1;
            ExpandResult {
                is_death: false,
                depth: 1,
                new_nodes: self.child_nodes
            }
        }
    }
}

/// Expect: If there is no hold piece, there are at least 2 pieces in the queue.
/// Otherwise there is at least 1 piece in the queue.
fn new_children(
    mut board: Board,
    opts: Options,
    evaluator: &impl Evaluator
) -> Vec<Child> {
    let mut children = vec![];
    let next = board.advance_queue().unwrap();
    let spawned = match FallingPiece::spawn(next, &board) {
        Some(s) => s,
        None => return children
    };

    // Placements for next piece
    for mv in crate::moves::find_moves(&board, spawned, opts.mode) {
        add_child(&mut children, &board, false, mv, evaluator);
    }

    if opts.use_hold {
        let mut board = board.clone();
        let hold = board.hold(next).unwrap_or_else(|| board.advance_queue().unwrap());
        if hold != next {
            if let Some(spawned) = FallingPiece::spawn(hold, &board) {
                // Placements for hold piece
                for mv in crate::moves::find_moves(&board, spawned, opts.mode) {
                    add_child(&mut children, &board, true, mv, evaluator);
                }
            }
        }
    }

    children.sort_by_key(|child| -child.tree.evaluation);
    for (i, child) in children.iter_mut().enumerate() {
        child.original_rank = i;
    }
    children
}

fn add_child(
    children: &mut Vec<Child>, board: &Board, hold: bool, mv: Placement, evaluator: &impl Evaluator
) {
    let mut board = board.clone();
    let can_be_hd = board.above_stack(&mv.location) &&
            board.column_heights().iter().all(|&y| y < 18);
    let lock = board.lock_piece(mv.location);
    if !lock.locked_out && !(can_be_hd && lock.placement_kind == PlacementKind::MiniTspin) {
        let move_time = mv.inputs.time + if hold { 1 } else { 0 };
        children.push(Child {
            tree: Tree::new(board, &lock, move_time, mv.location.kind.0, evaluator),
            original_rank: 0,
            hold, mv, lock
        })
    }
}

struct ExpandResult {
    depth: usize,
    new_nodes: usize,
    is_death: bool
}

impl TreeKind {
    fn into_best_child(self) -> Result<Child, TreeKind> {
        match self {
            TreeKind::Known(children) => if children.is_empty() {
                Err(TreeKind::Known(children))
            } else {
                Ok(children.into_iter().next().unwrap())
            },
            TreeKind::Unknown(_) => Err(self),
        }
    }

    fn get_best_child(&self) -> Option<&Child> {
        match self {
            TreeKind::Known(children) => children.first(),
            TreeKind::Unknown(_) => None
        }
    }

    fn get_plan(&self, into: &mut Vec<(Placement, LockResult)>) {
        match self {
            TreeKind::Known(children) => if let Some(mv) = children.first() {
                into.push((mv.mv.clone(), mv.lock.clone()));
                mv.tree.get_plan(into);
            }
            _ => {}
        }
    }

    fn get_moves_and_evaluations(&self) -> Vec<(FallingPiece, i32)> {
        match self {
            TreeKind::Known(children) => children.iter()
                .map(|c| (c.mv.location, c.tree.evaluation))
                .collect(),
            _ => vec![]
        }
    }

    fn evaluation(&self) -> i32 {
        match self {
            TreeKind::Known(children) => children.first().unwrap().tree.evaluation,
            TreeKind::Unknown(speculation) => {
                let mut sum = 0;
                let mut n = 0;
                let mut deaths = 0;
                for children in speculation.iter().filter_map(|(_, c)| c.as_ref()) {
                    match children.first() {
                        Some(c) => {
                            n += 1;
                            sum += c.tree.evaluation;
                        }
                        None => deaths += 1,
                    }
                }
                let avg_value = sum / n;
                sum += (avg_value - 1000) * deaths;
                sum / (n + deaths)
            }
        }
    }

    /// Returns is_death
    fn add_next_piece(&mut self, piece: Piece, opts: Options) -> bool {
        match self {
            TreeKind::Known(children) => {
                children.retain_mut(|child|
                    !child.tree.add_next_piece(piece, opts)
                );
                if children.is_empty() {
                    true
                } else {
                    children.sort_by_key(|child| -child.tree.evaluation);
                    false
                }
            }
            TreeKind::Unknown(speculation) => {
                let mut now_known = vec![];
                if speculation[piece].is_none() {
                    let mut error = format!(
                        "Invalid next piece added: {}, expected one of ", piece.to_char()
                    );
                    for (p, v) in speculation.iter() {
                        if v.is_some() {
                            error.push(p.to_char());
                            error.push_str(", ");
                        }
                    }
                    panic!("{}", error);
                }
                std::mem::swap(speculation[piece].as_mut().unwrap(), &mut now_known);
                let is_death = now_known.is_empty();
                *self = TreeKind::Known(now_known);
                is_death
            }
        }
    }

    fn expand(
        &mut self,
        opts: Options,
        evaluator: &impl Evaluator
    ) -> ExpandResult {
        let to_expand = match self {
            TreeKind::Known(children) => children,
            TreeKind::Unknown(speculation) => {
                let mut pieces = ArrayVec::<[Piece; 7]>::new();
                for (piece, children) in speculation.iter() {
                    if let Some(children) = children {
                        if !children.is_empty() {
                            pieces.push(piece);
                        }
                    }
                }
                speculation[*pieces.choose(&mut thread_rng()).unwrap()].as_mut().unwrap()
            }
        };
        if to_expand.is_empty() {
            return ExpandResult {
                depth: 0,
                new_nodes: 0,
                is_death: true
            }
        }

        let min = to_expand.last().unwrap().tree.evaluation;
        let weights = to_expand.iter()
            .enumerate()
            .map(|(i, c)| {
                let e = (c.tree.evaluation - min) as i64;
                e * e / (i + 1) as i64 + 1
            });
        let sampler = rand::distributions::WeightedIndex::new(weights).unwrap();
        let index = thread_rng().sample(sampler);

        let result = to_expand[index].tree.expand(opts, evaluator);
        if result.is_death {
            to_expand.remove(index);
            match self {
                TreeKind::Known(children) => if children.is_empty() {
                    return ExpandResult {
                        is_death: true,
                        depth: result.depth + 1,
                        ..result
                    }
                }
                TreeKind::Unknown(speculation) => if speculation.iter()
                        .all(|(_, c)| c.as_ref().map(Vec::is_empty).unwrap_or(true)) {
                    return ExpandResult {
                        is_death: true,
                        depth: result.depth + 1,
                        ..result
                    }
                }
            }
            ExpandResult {
                is_death: false,
                depth: result.depth + 1,
                ..result
            }
        } else {
            to_expand.sort_by_key(|c| -c.tree.evaluation);
            ExpandResult {
                depth: result.depth + 1,
                ..result
            }
        }
    }
}
