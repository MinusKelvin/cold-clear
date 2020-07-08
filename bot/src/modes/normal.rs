use serde::{ Serialize, Deserialize };
use enum_map::EnumMap;
use libtetris::*;
// use crate::tree::{ ChildData, TreeState, NodeId };
use crate::dag::{ DagState, NodeId, ChildData };
use crate::{ Options, Info };
pub use crate::moves::Move;
use crate::evaluation::Evaluator;

pub struct BotState<E: Evaluator> {
    tree: DagState<E::Value, E::Reward>,
    options: Options,
    forced_analysis_lines: Vec<Vec<FallingPiece>>,
    pub outstanding_thinks: u32
}

#[derive(Serialize, Deserialize)]
pub struct Thinker {
    node: NodeId,
    board: Board,
    options: Options,
}

#[derive(Serialize, Deserialize)]
pub enum ThinkResult<V, R> {
    Known(NodeId, Vec<ChildData<V, R>>),
    Speculated(NodeId, EnumMap<Piece, Option<Vec<ChildData<V, R>>>>),
    Unmark(NodeId)
}

impl<E: Evaluator> BotState<E> {
    pub fn new(board: Board, options: Options) -> Self {
        BotState {
            tree: DagState::new(board, options.use_hold),
            options,
            forced_analysis_lines: vec![],
            outstanding_thinks: 0
        }
    }

    /// Prepare a thinking cycle.
    /// 
    /// Returns `Err(true)` if a thinking cycle can be preformed, but it couldn't find 
    pub fn think(&mut self) -> Result<Thinker, bool> {
        if (!self.min_thinking_reached() || self.tree.nodes() < self.options.max_nodes)
                && !self.tree.is_dead() {
            if let Some((node, board)) = self.tree.find_and_mark_leaf(
                &mut self.forced_analysis_lines
            ) {
                self.outstanding_thinks += 1;
                return Ok(Thinker {
                    node, board,
                    options: self.options,
                });
            } else {
                return Err(true)
            }
        } else {
            return Err(false)
        }
    }

    pub fn finish_thinking(&mut self, result: ThinkResult<E::Value, E::Reward>) {
        self.outstanding_thinks -= 1;
        match result {
            ThinkResult::Known(node, children) => self.tree.update_known(node, children),
            ThinkResult::Speculated(node, children) => self.tree.update_speculated(node, children),
            ThinkResult::Unmark(node) => self.tree.unmark(node)
        }
    }

    pub fn is_dead(&self) -> bool {
        self.tree.is_dead()
    }

    /// Adds a new piece to the queue.
    pub fn add_next_piece(&mut self, piece: Piece) {
        self.tree.add_next_piece(piece);
    }

    pub fn reset(&mut self, field: [[bool; 10]; 40], b2b: bool, combo: u32) {
        let plan = self.tree.get_plan();
        if let Some(garbage_lines) = self.tree.reset(field, b2b, combo) {
            for path in &mut self.forced_analysis_lines {
                for mv in path {
                    mv.y += garbage_lines;
                }
            }
            let mut prev_best_path = vec![];
            for mv in plan {
                let mut mv = mv.0;
                mv.y += garbage_lines;
                prev_best_path.push(mv);
            }
            self.forced_analysis_lines.push(prev_best_path);
        } else {
            self.forced_analysis_lines.clear();
        }
    }

    pub fn min_thinking_reached(&self) -> bool {
        self.tree.nodes() > self.options.min_nodes && self.forced_analysis_lines.is_empty()
    }

    pub fn next_move(&mut self, eval: &E, incoming: u32, f: impl FnOnce(Move, Info)) -> bool {
        if !self.min_thinking_reached() {
            return false
        }

        let candidates = self.tree.get_next_candidates();
        if candidates.is_empty() {
            return false
        }
        let child = eval.pick_move(candidates, incoming);

        let plan = self.tree.get_plan();

        let info = Info {
            nodes: self.tree.nodes(),
            depth: self.tree.depth() as u32,
            original_rank: child.original_rank,
            evaluation_result: eval.get_result(&child.evaluation),
            plan,
        };

        let inputs = crate::moves::find_moves(
            self.tree.board(),
            self.options.spawn_rule.spawn(child.mv.kind.0, self.tree.board()).unwrap(),
            self.options.mode
        ).into_iter().find(|p| p.location == child.mv).unwrap().inputs;
        let mv = Move {
            hold: child.hold,
            inputs: inputs.movements,
            expected_location: child.mv
        };

        f(mv, info);

        self.tree.advance_move(child.mv);

        true
    }

    pub fn force_analysis_line(&mut self, path: Vec<FallingPiece>) {
        self.forced_analysis_lines.push(path);
    }
}

impl Thinker {
    pub fn think<E: Evaluator>(self, eval: &E) -> ThinkResult<E::Value, E::Reward> {
        if let Err(possibilities) = self.board.get_next_piece() {
            // Next unknown (implies hold is known) => Speculate
            if self.options.speculate {
                let mut children = EnumMap::new();
                for p in possibilities {
                    let mut b = self.board.clone();
                    b.add_next_piece(p);
                    children[p] = Some(self.make_children(b, eval));
                }
                ThinkResult::Speculated(self.node, children)
            } else {
                ThinkResult::Unmark(self.node)
            }
        } else {
            if self.options.use_hold && self.board.hold_piece.is_none() &&
                    self.board.get_next_next_piece().is_none() {
                // Next known, hold unknown => Speculate
                if self.options.speculate {
                    let mut children = EnumMap::new();
                    let possibilities = {
                        let mut b = self.board.clone();
                        b.advance_queue();
                        b.get_next_piece().unwrap_err()
                    };
                    for p in possibilities {
                        let mut b = self.board.clone();
                        b.add_next_piece(p);
                        children[p] = Some(self.make_children(b, eval));
                    }
                    ThinkResult::Speculated(self.node, children)
                } else {
                    ThinkResult::Unmark(self.node)
                }
            } else {
                // Next and hold known
                let children = self.make_children(self.board.clone(), eval);
                ThinkResult::Known(self.node, children)
            }
        }
    }

    fn make_children<E: Evaluator>(
        &self, mut board: Board, eval: &E
    ) -> Vec<ChildData<E::Value, E::Reward>> {
        let mut children = vec![];

        let next = board.advance_queue().unwrap();
        let spawned = match self.options.spawn_rule.spawn(next, &board) {
            Some(spawned) => spawned,
            None => return children
        };

        self.add_children(&mut children, &board, eval, spawned, false);

        if self.options.use_hold {
            let hold = board.hold(next).unwrap_or_else(|| board.advance_queue().unwrap());
            if hold == next {
                return children
            }
            let spawned = match self.options.spawn_rule.spawn(hold, &board) {
                Some(spawned) => spawned,
                None => return children
            };
        
            self.add_children(&mut children, &board, eval, spawned, true);
        }

        children
    }

    fn add_children<E: Evaluator>(
        &self,
        children: &mut Vec<ChildData<E::Value, E::Reward>>,
        board: &Board,
        eval: &E,
        spawned: FallingPiece,
        hold: bool
    ) {
        for mv in crate::moves::find_moves(&board, spawned, self.options.mode) {
            let can_be_hd = board.above_stack(&mv.location) &&
            board.column_heights().iter().all(|&y| y < 18);
            let mut result = board.clone();
            let lock = result.lock_piece(mv.location);
            // Don't add deaths by lock out, don't add useless mini tspins
            if !lock.locked_out && !(can_be_hd && lock.placement_kind == PlacementKind::MiniTspin) {
                let move_time = mv.inputs.time + if hold { 1 } else { 0 };
                let (evaluation, reward) = eval.evaluate(
                    &lock, &result, move_time, spawned.kind.0
                );
                children.push(ChildData {
                    evaluation,
                    reward,
                    board: result,
                    mv: mv.location
                });
            }
        }
    }
}