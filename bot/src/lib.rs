use std::sync::mpsc::{ Sender, Receiver, TryRecvError, TrySendError, channel, sync_channel };
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
    pub min_nodes: usize,
    pub max_nodes: usize,
    pub threads: usize
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: crate::moves::MovementMode::ZeroG,
            use_hold: true,
            speculate: true,
            min_nodes: 0,
            max_nodes: std::usize::MAX,
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
    pub fn request_next_move(&mut self) {
        if self.send.send(BotMsg::NextMove).is_err() {
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
}

enum BotMsg {
    Reset {
        field: [[bool; 10]; 40],
        b2b: bool,
        combo: u32
    },
    NewPiece(Piece),
    NextMove
}

pub struct BotState<E: Evaluator> {
    tree: TreeState,
    options: Options,
    eval: Arc<E>,
}

pub struct Thinker<E: Evaluator> {
    node: NodeId,
    board: Board,
    options: Options,
    eval: Arc<E>
}

pub enum ThinkResult {
    Known(NodeId, Vec<ChildData>),
    Speculated(NodeId, EnumMap<Piece, Option<Vec<ChildData>>>),
    Unmark(NodeId)
}

impl<E: Evaluator> BotState<E> {
    pub fn new(board: Board, options: Options, eval: E) -> Self {
        BotState {
            tree: TreeState::create(board, options.use_hold),
            options,
            eval: Arc::new(eval)
        }
    }

    /// Prepare a thinking cycle.
    /// 
    /// Returns `Err(true)` if a thinking cycle can be preformed, but it couldn't find 
    pub fn think(&mut self) -> Result<Thinker<E>, bool> {
        if self.tree.nodes < self.options.max_nodes && !self.tree.is_dead() {
            if let Some((node, board)) = self.tree.find_and_mark_leaf() {
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

    pub fn finish_thinking(&mut self, result: ThinkResult) {
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
        self.tree.reset(field, b2b, combo);
    }

    pub fn min_thinking_reached(&self) -> bool {
        self.tree.nodes > self.options.min_nodes
    }

    pub fn next_move(&mut self, f: impl FnOnce(Move, Info)) -> bool {
        if self.tree.nodes < self.options.min_nodes {
            return false
        }

        let child = if let Some(child) = self.tree.best_move() {
            child
        } else {
            return false
        };

        let mut plan = vec![(child.mv, child.lock.clone())];
        let mut next = child.node;
        while let Some(Some(tree::Children::Known(children))) = self.tree.get_children(next) {
            let child = &children[0];
            plan.push((child.mv, child.lock.clone()));
            next = child.node;
        }

        let info = Info {
            nodes: self.tree.nodes,
            depth: self.tree.depth(),
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

        self.tree.advance_move();

        true
    }
}

impl<E: Evaluator> Thinker<E> {
    pub fn think(self) -> ThinkResult {
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

    fn make_children(&self, mut board: Board) -> Vec<ChildData> {
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
        &self, children: &mut Vec<ChildData>, board: &Board, spawned: FallingPiece, hold: bool
    ) {
        for mv in moves::find_moves(&board, spawned, self.options.mode) {
            let can_be_hd = board.above_stack(&mv.location) &&
            board.column_heights().iter().all(|&y| y < 18);
            let mut result = board.clone();
            let lock = result.lock_piece(mv.location);
            // Don't add deaths by lock out, don't add useless mini tspins
            if !lock.locked_out && !(can_be_hd && lock.placement_kind == PlacementKind::MiniTspin) {
                let move_time = mv.inputs.time + if hold { 1 } else { 0 };
                let evaluated = self.eval.evaluate(&lock, &result, move_time, spawned.kind.0);
                children.push(ChildData {
                    accumulated: evaluated.accumulated,
                    evaluation: evaluated.transient,
                    board: result,
                    hold,
                    mv: mv.location,
                    lock,
                });
            }
        }
    }
}

fn run(
    recv: Receiver<BotMsg>,
    send: Sender<(Move, Info)>,
    board: Board,
    evaluator: impl Evaluator + 'static,
    options: Options
) {
    if options.threads == 0 {
        panic!("Invalid number of threads: 0");
    }

    let mut bot = BotState::new(board, options, evaluator);

    let pool = rayon::ThreadPoolBuilder::new().num_threads(options.threads).build().unwrap();

    let (result_send, result_recv) = channel();
    let mut tasks = 0;

    while !bot.is_dead() {
        match recv.try_recv() {
            Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
            Ok(BotMsg::NewPiece(piece)) => bot.add_next_piece(piece),
            Ok(BotMsg::Reset { field, b2b, combo }) => bot.reset(field, b2b, combo),
            Ok(BotMsg::NextMove) =>
                while !bot.next_move(|mv, info| { send.send((mv, info)).ok(); }) {
                    if bot.is_dead() {
                        break
                    }
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
    pub nodes: usize,
    pub depth: usize,
    pub original_rank: usize,
    pub plan: Vec<(FallingPiece, LockResult)>
}
