use std::sync::mpsc::{ Sender, Receiver, TryRecvError, channel };
use std::sync::Arc;
use serde::{ Serialize, Deserialize };
use enum_map::EnumMap;

pub mod evaluation;
pub mod moves;
mod tree;

use libtetris::*;
use crate::tree::{ ChildData, TreeState, NodeId };
use crate::moves::Move;
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

pub struct Interface {
    send: Sender<BotMsg>,
    recv: Receiver<(Move, Info)>,
    dead: bool,
    mv: Option<(Move, Info)>
}

impl Interface {
    /// Launches a bot thread with the specified starting board and options.
    pub fn launch(
        board: Board, options: Options, evaluator: impl Evaluator + Send + 'static
    ) -> Self {
        let (bot_send, recv) = channel();
        let (send, bot_recv) = channel();
        std::thread::spawn(move || run(bot_recv, bot_send, board, evaluator, options));

        Interface {
            send, recv, dead: false, mv: None
        }
    }

    /// Returns true if all possible piece placement sequences result in death, or the bot thread
    /// crashed.
    pub fn is_dead(&self) -> bool {
        self.dead
    }

    fn poll_bot(&mut self) {
        loop {
            match self.recv.try_recv() {
                Ok(mv) => self.mv = Some(mv),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.dead = true;
                    break
                }
            }
        }
    }

    /// Request the bot to provide a move as soon as possible.
    /// 
    /// In most cases, "as soon as possible" is a very short amount of time, and is only longer if
    /// the provided lower limit on thinking has not been reached yet or if the bot cannot provide
    /// a move yet, usually because it lacks information on the next pieces.
    /// 
    /// For example, in a game with zero piece previews and hold enabled, the bot will never be able
    /// to provide the first move because it cannot know what piece it will be placing if it chooses
    /// to hold. Another example: in a game with zero piece previews and hold disabled, the bot
    /// will only be able to provide a move after the current piece spawns and you provide the piece
    /// information to the bot using `add_next_piece`.
    /// 
    /// It is recommended that you call this function the frame before the piece spawns so that the
    /// bot has time to finish its current thinking cycle and supply the move.
    /// 
    /// Once a move is chosen, the bot will update its internal state to the result of the piece
    /// being placed correctly and the move will become available by calling `poll_next_move`.
    pub fn request_next_move(&mut self, incoming: u32) {
        if self.send.send(BotMsg::NextMove(incoming)).is_err() {
            self.dead = true;
        }
    }

    /// Checks to see if the bot has provided the previously requested move yet.
    /// 
    /// The returned move contains both a path and the expected location of the placed piece. The
    /// returned path is reasonably good, but you might want to use your own pathfinder to, for
    /// example, exploit movement intricacies in the game you're playing.
    /// 
    /// If the piece couldn't be placed in the expected location, you must call `reset` to reset the
    /// game field, back-to-back status, and combo values.
    pub fn poll_next_move(&mut self) -> Option<(Move, Info)> {
        self.poll_bot();
        self.mv.take()
    }

    /// Adds a new piece to the end of the queue.
    /// 
    /// If speculation is enabled, the piece *must* be in the bag. For example, if in the current
    /// bag you've provided the sequence IJOZT, then the next time you call this function you can
    /// only provide either an L or an S piece.
    pub fn add_next_piece(&mut self, piece: Piece) {
        if self.send.send(BotMsg::NewPiece(piece)).is_err() {
            self.dead = true;
        }
    }

    /// Resets the playfield, back-to-back status, and combo count.
    /// 
    /// This should only be used when garbage is received or when your client could not place the
    /// piece in the correct position for some reason (e.g. 15 move rule), since this forces the
    /// bot to throw away previous computations.
    /// 
    /// Note: combo is not the same as the displayed combo in guideline games. Here, it is the
    /// number of consecutive line clears achieved. So, generally speaking, if "x Combo" appears
    /// on the screen, you need to use x+1 here.
    pub fn reset(&mut self, field: [[bool; 10]; 40], b2b_active: bool, combo: u32) {
        if self.send.send(BotMsg::Reset {
            field, b2b: b2b_active, combo
        }).is_err() {
            self.dead = true;
        }
    }

    /// Specifies a line that Cold Clear should analyze before making any moves. The path is
    /// sensitive to T-spin status.
    pub fn force_analysis_line(&mut self, path: Vec<FallingPiece>) {
        if self.send.send(BotMsg::ForceAnalysisLine(path)).is_err() {
            self.dead = true;
        }
    }
}

enum BotMsg {
    Reset {
        field: [[bool; 10]; 40],
        b2b: bool,
        combo: u32
    },
    NewPiece(Piece),
    NextMove(u32),
    ForceAnalysisLine(Vec<FallingPiece>)
}

pub struct BotState<E: Evaluator> {
    tree: TreeState<E::Value, E::Reward>,
    options: Options,
    eval: Arc<E>,
    forced_analysis_lines: Vec<Vec<FallingPiece>>
}

pub struct Thinker<E: Evaluator> {
    node: NodeId,
    board: Board,
    options: Options,
    eval: Arc<E>
}

pub enum ThinkResult<E: Evaluator> {
    Known(NodeId, Vec<ChildData<E::Value, E::Reward>>),
    Speculated(NodeId, EnumMap<Piece, Option<Vec<ChildData<E::Value, E::Reward>>>>),
    Unmark(NodeId)
}

impl<E: Evaluator> BotState<E> {
    pub fn new(board: Board, options: Options, eval: E) -> Self {
        BotState {
            tree: TreeState::create(board, options.use_hold),
            options,
            eval: Arc::new(eval),
            forced_analysis_lines: vec![],
        }
    }

    /// Prepare a thinking cycle.
    /// 
    /// Returns `Err(true)` if a thinking cycle can be preformed, but it couldn't find 
    pub fn think(&mut self) -> Result<Thinker<E>, bool> {
        if self.tree.nodes < self.options.max_nodes && !self.tree.is_dead() {
            if let Some((node, board)) = self.tree.find_and_mark_leaf(
                &mut self.forced_analysis_lines
            ) {
                return Ok(Thinker {
                    node, board,
                    options: self.options,
                    eval: Arc::clone(&self.eval)
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
        if self.tree.nodes < self.options.min_nodes {
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

fn run(
    recv: Receiver<BotMsg>,
    send: Sender<(Move, Info)>,
    mut board: Board,
    evaluator: impl Evaluator + 'static,
    options: Options
) {
    if options.threads == 0 {
        panic!("Invalid number of threads: 0");
    }

    let mut do_move = None;

    while board.next_queue().next().is_none() {
        match recv.recv() {
            Err(_) => return,
            Ok(BotMsg::NewPiece(piece)) => board.add_next_piece(piece),
            Ok(BotMsg::Reset { field, b2b, combo }) =>{
                board.set_field(field);
                board.combo = combo;
                board.b2b_bonus = b2b;
            }
            Ok(BotMsg::NextMove(incoming)) => do_move = Some(incoming),
            Ok(BotMsg::ForceAnalysisLine(path)) => {}
        }
    }

    let mut bot = BotState::new(board, options, evaluator);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(options.threads as usize)
        .build().unwrap();

    let (result_send, result_recv) = channel();
    let mut tasks = 0;

    while !bot.is_dead() {
        match recv.try_recv() {
            Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
            Ok(BotMsg::NewPiece(piece)) => bot.add_next_piece(piece),
            Ok(BotMsg::Reset { field, b2b, combo }) => bot.reset(field, b2b, combo),
            Ok(BotMsg::NextMove(incoming)) => do_move = Some(incoming),
            Ok(BotMsg::ForceAnalysisLine(path)) => bot.force_analysis_line(path)
        }

        if let Some(incoming) = do_move {
            if bot.next_move(incoming, |mv, info| { send.send((mv, info)).ok(); }) {
                do_move = None;
            }
        }

        if tasks < 2*options.threads {
            if let Ok(thinker) = bot.think() {
                let result_send = result_send.clone();
                pool.spawn_fifo(move || {
                    result_send.send(thinker.think()).ok();
                });
                tasks += 1;
            }
        }

        if tasks == 2*options.threads {
            if let Ok(result) = result_recv.recv() {
                tasks -= 1;
                bot.finish_thinking(result);
            }
        }
        for result in result_recv.try_iter() {
            tasks -= 1;
            bot.finish_thinking(result);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Info {
    pub nodes: u32,
    pub depth: u32,
    pub original_rank: u32,
    pub plan: Vec<(FallingPiece, LockResult)>
}
