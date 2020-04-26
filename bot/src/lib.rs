use serde::{ Serialize, Deserialize };
use enum_map::EnumMap;

pub mod evaluation;
pub mod moves;
mod tree;

#[cfg(not(target_arch = "wasm32"))]
mod desktop;
#[cfg(not(target_arch = "wasm32"))]
pub use desktop::Interface;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::Interface;

use libtetris::*;
use crate::tree::{ ChildData, TreeState, NodeId };
pub use crate::moves::Move;
use crate::evaluation::Evaluator;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub mode: crate::moves::MovementMode,
    pub use_hold: bool,
    pub speculate: bool,
    pub min_nodes: u32,
    pub max_nodes: u32,
    pub threads: u32
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: crate::moves::MovementMode::ZeroG,
            use_hold: true,
            speculate: true,
            min_nodes: 0,
            max_nodes: 4_000_000_000,
            threads: 1
        }
    }
}

pub struct BotState<E: Evaluator + Clone> {
    tree: TreeState<E::Value, E::Reward>,
    options: Options,
    eval: E,
    forced_analysis_lines: Vec<Vec<FallingPiece>>
}

#[derive(Serialize, Deserialize)]
pub struct Thinker<E: Evaluator> {
    node: NodeId,
    board: Board,
    options: Options,
    eval: E
}

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "E::Value: Serialize, E::Reward: Serialize"))]
#[serde(bound(deserialize = "E::Value: Deserialize<'de>, E::Reward: Deserialize<'de>"))]
pub enum ThinkResult<E: Evaluator> {
    Known(NodeId, Vec<ChildData<E::Value, E::Reward>>),
    Speculated(NodeId, EnumMap<Piece, Option<Vec<ChildData<E::Value, E::Reward>>>>),
    Unmark(NodeId)
}

impl<E: Evaluator + Clone> BotState<E> {
    pub fn new(board: Board, options: Options, eval: E) -> Self {
        BotState {
            tree: TreeState::create(board, options.use_hold),
            options, eval,
            forced_analysis_lines: vec![],
        }
    }

    /// Prepare a thinking cycle.
    /// 
    /// Returns `Err(true)` if a thinking cycle can be preformed, but it couldn't find 
    pub fn think(&mut self) -> Result<Thinker<E>, bool> {
        if (!self.min_thinking_reached() || self.tree.nodes < self.options.max_nodes)
                && !self.tree.is_dead() {
            if let Some((node, board)) = self.tree.find_and_mark_leaf(
                &mut self.forced_analysis_lines
            ) {
                return Ok(Thinker {
                    node, board,
                    options: self.options,
                    eval: self.eval.clone()
                });
            } else {
                return Err(true)
            }
        } else {
            return Err(false)
        }
    }

    pub fn finish_thinking(&mut self, result: ThinkResult<E>) {
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
        self.tree.nodes > self.options.min_nodes && self.forced_analysis_lines.is_empty()
    }

    pub fn next_move(&mut self, incoming: u32, f: impl FnOnce(Move, Info)) -> bool {
        if !self.min_thinking_reached() {
            return false
        }

        let candidates = self.tree.get_next_candidates();
        if candidates.is_empty() {
            return false
        }
        let child = self.eval.pick_move(candidates, incoming);

        let plan = self.tree.get_plan();

        let info = Info {
            nodes: self.tree.nodes,
            depth: self.tree.depth() as u32,
            original_rank: child.original_rank,
            plan,
        };

        let inputs = moves::find_moves(
            &self.tree.board,
            FallingPiece::spawn(child.mv.kind.0, &self.tree.board).unwrap(),
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

impl<E: Evaluator> Thinker<E> {
    pub fn think(self) -> ThinkResult<E> {
        if let Err(possibilities) = self.board.get_next_piece() {
            // Next unknown (implies hold is known) => Speculate
            if self.options.speculate {
                let mut children = EnumMap::new();
                for p in possibilities {
                    let mut b = self.board.clone();
                    b.add_next_piece(p);
                    children[p] = Some(self.make_children(b));
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
                        children[p] = Some(self.make_children(b));
                    }
                    ThinkResult::Speculated(self.node, children)
                } else {
                    ThinkResult::Unmark(self.node)
                }
            } else {
                // Next and hold known
                let children = self.make_children(self.board.clone());
                ThinkResult::Known(self.node, children)
            }
        }
    }

    fn make_children(&self, mut board: Board) -> Vec<ChildData<E::Value, E::Reward>> {
        let mut children = vec![];

        let next = board.advance_queue().unwrap();
        let spawned = match FallingPiece::spawn(next, &board) {
            Some(spawned) => spawned,
            None => return children
        };

        self.add_children(&mut children, &board, spawned, false);

        if self.options.use_hold {
            let hold = board.hold(next).unwrap_or_else(|| board.advance_queue().unwrap());
            if hold == next {
                return children
            }
            let spawned = match FallingPiece::spawn(hold, &board) {
                Some(spawned) => spawned,
                None => return children
            };
        
            self.add_children(&mut children, &board, spawned, true);
        }

        children
    }

    fn add_children(
        &self,
        children: &mut Vec<ChildData<E::Value, E::Reward>>,
        board: &Board,
        spawned: FallingPiece,
        hold: bool
    ) {
        for mv in moves::find_moves(&board, spawned, self.options.mode) {
            let can_be_hd = board.above_stack(&mv.location) &&
            board.column_heights().iter().all(|&y| y < 18);
            let mut result = board.clone();
            let lock = result.lock_piece(mv.location);
            // Don't add deaths by lock out, don't add useless mini tspins
            if !lock.locked_out && !(can_be_hd && lock.placement_kind == PlacementKind::MiniTspin) {
                let move_time = mv.inputs.time + if hold { 1 } else { 0 };
                let (evaluation, accumulated) = self.eval.evaluate(
                    &lock, &result, move_time, spawned.kind.0
                );
                children.push(ChildData {
                    evaluation,
                    accumulated,
                    board: result,
                    hold,
                    mv: mv.location
                });
            }
        }
    }
}

struct AsyncBotState<E: Evaluator + Clone> {
    bot: BotState<E>,
    options: Options,
    do_move: Option<u32>,
    tasks: u32
}

impl<E: Evaluator + Clone> AsyncBotState<E> {
    pub fn new(board: Board, options: Options, evaluator: E) -> Self {
        AsyncBotState {
            bot: BotState::new(board, options, evaluator),
            options,
            do_move: None,
            tasks: 0
        }
    }

    pub fn think_done(&mut self, think_result: ThinkResult<E>) {
        self.tasks -= 1;
        self.bot.finish_thinking(think_result);
    }

    pub fn message(&mut self, msg: BotMsg) {
        match msg {
            BotMsg::Reset { field, b2b, combo } => self.bot.reset(field, b2b, combo),
            BotMsg::NewPiece(piece) => self.bot.add_next_piece(piece),
            BotMsg::NextMove(incoming) => self.do_move = Some(incoming),
            BotMsg::ForceAnalysisLine(path) => self.bot.force_analysis_line(path)
        }
    }

    pub fn think(&mut self, send_move: impl FnOnce(Move, Info)) -> (Vec<Thinker<E>>, bool) {
        if let Some(incoming) = self.do_move {
            if self.bot.next_move(incoming, send_move) {
                self.do_move = None;
            }
        }

        let mut thinks = vec![];
        let mut can_think = false;
        for _ in 0..10 {
            if self.tasks >= 2*self.options.threads {
                can_think = true;
                break
            }
            match self.bot.think() {
                Ok(thinker) => {
                    thinks.push(thinker);
                    self.tasks += 1;
                }
                Err(false) => break,
                Err(true) => can_think = true
            }
        }
        (thinks, can_think)
    }

    pub fn is_dead(&self) -> bool {
        self.bot.is_dead()
    }

    pub fn should_wait_for_think(&self) -> bool {
        self.tasks == self.options.threads * 2
    }
}

use serde_big_array::big_array;
big_array!( BigArray; 40, );

#[derive(Serialize, Deserialize)]
enum BotMsg {
    Reset {
        #[serde(with = "BigArray")]
        field: [[bool; 10]; 40],
        b2b: bool,
        combo: u32
    },
    NewPiece(Piece),
    NextMove(u32),
    ForceAnalysisLine(Vec<FallingPiece>)
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Info {
    pub nodes: u32,
    pub depth: u32,
    pub original_rank: u32,
    pub plan: Vec<(FallingPiece, LockResult)>
}

pub enum BotPollState {
    Waiting,
    Dead
}