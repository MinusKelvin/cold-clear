use std::sync::mpsc::{ Sender, Receiver, TryRecvError, channel };
use serde::{ Serialize, Deserialize };

pub mod evaluation;
mod misa;
pub mod moves;
mod tree;

use libtetris::*;
use crate::tree::Tree;
use crate::moves::{ Move, Placement };
use crate::evaluation::Evaluator;

#[derive(Copy, Clone, Debug)]
pub struct Options {
    pub mode: crate::moves::MovementMode,
    pub use_hold: bool,
    pub speculate: bool,
    pub min_nodes: usize,
    pub max_nodes: usize,
    pub gamma: (i32, i32)
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: crate::moves::MovementMode::ZeroG,
            use_hold: true,
            speculate: true,
            min_nodes: 0,
            max_nodes: std::usize::MAX,
            gamma: (1, 1)
        }
    }
}

pub struct Interface {
    send: Sender<BotMsg>,
    recv: Receiver<BotResult>,
    dead: bool,
    mv: Option<Move>,
    info: Option<Info>
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
            send, recv, dead: false, mv: None, info: None
        }
    }

    pub fn misa_glue(board: Board) -> Self {
        let (bot_send, recv) = channel();
        let (send, bot_recv) = channel();
        std::thread::spawn(move || misa::glue(bot_recv, bot_send, board));

        Interface {
            send, recv, dead: false, mv: None, info: None
        }
    }

    pub fn misa_prepare_next_move(&mut self) {
        if self.send.send(BotMsg::PrepareNextMove).is_err() {
            self.dead = true;
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
                Ok(BotResult::Move(mv)) => self.mv = Some(mv),
                Ok(BotResult::Info(info)) => self.info = Some(info),
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
    pub fn poll_next_move(&mut self) -> Option<Move> {
        self.poll_bot();
        self.mv.take()
    }

    pub fn poll_info(&mut self) -> Option<Info> {
        self.poll_bot();
        self.info.take()
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
    NextMove,
    PrepareNextMove
}

#[derive(Debug)]
enum BotResult {
    Move(Move),
    Info(Info)
}

pub struct BotState<E: Evaluator> {
    tree: Tree,
    options: Options,
    dead: bool,
    eval: E,
}

impl<E: Evaluator> BotState<E> {
    pub fn new(board: Board, options: Options, eval: E) -> Self {
        BotState {
            dead: false,
            tree: Tree::new(board, &Default::default(), 0, &eval),
            options,
            eval
        }
    }

    /// Perform a thinking cycle.
    /// 
    /// Returns true if a thinking cycle was performed. If a thinking cycle was not performed,
    /// calling this function again will not perform a thinking cycle either.
    pub fn think(&mut self) -> bool {
        if self.tree.child_nodes < self.options.max_nodes &&
                self.tree.board.next_queue().count() > 0 &&
                !self.dead {
            if self.tree.extend(self.options, &self.eval) {
                self.dead = true;
            }
            true
        } else {
            false
        }
    }

    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// Adds a new piece to the queue.
    pub fn add_next_piece(&mut self, piece: Piece) {
        if self.tree.add_next_piece(piece, self.options) {
            self.dead = true;
        }
    }

    pub fn reset(&mut self, field: [[bool; 10]; 40], b2b: bool, combo: u32) {
        let mut board = Board::new();
        std::mem::swap(&mut board, &mut self.tree.board);
        board.set_field(field);
        board.combo = combo;
        board.b2b_bonus = b2b;
        self.tree = Tree::new(board, &Default::default(), 0, &self.eval);
    }

    pub fn min_thinking_reached(&self) -> bool {
        self.tree.child_nodes > self.options.min_nodes
    }

    pub fn next_move(&mut self) -> Option<(Move, Info)> {
        if !self.min_thinking_reached() {
            return None;
        }

        let moves_considered = self.tree.child_nodes;
        let mut tree = Tree::empty();
        std::mem::swap(&mut tree, &mut self.tree);
        match tree.into_best_child() {
            Ok(child) => {
                let mut plan = vec![(child.mv.clone(), child.lock.clone())];
                child.tree.get_plan(&mut plan);
                let info = Info {
                    evaluation: child.tree.evaluation,
                    nodes: moves_considered,
                    depth: child.tree.depth+1,
                    plan
                };
                let mv = Move {
                    hold: child.hold,
                    inputs: child.mv.inputs.movements,
                    expected_location: child.mv.location
                };
                self.tree = child.tree;
                Some((mv, info))
            }
            Err(t) => {
                self.tree = t;
                None
            }
        }
    }

    pub fn get_possible_next_moves_and_evaluations(&self) -> Vec<(FallingPiece, i32)> {
        self.tree.get_moves_and_evaluations()
    }
}

fn run(
    recv: Receiver<BotMsg>,
    send: Sender<BotResult>,
    board: Board,
    evaluator: impl Evaluator,
    options: Options
) {
    let mut bot = BotState::new(board, options, evaluator);

    let mut do_move = false;
    let mut can_think = true;
    while !bot.is_dead() {
        let result = if can_think {
            recv.try_recv()
        } else {
            recv.recv().map_err(|_| TryRecvError::Disconnected)
        };
        match result {
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
            Ok(BotMsg::NewPiece(piece)) => bot.add_next_piece(piece),
            Ok(BotMsg::Reset { field, b2b, combo }) => bot.reset(field, b2b, combo),
            Ok(BotMsg::NextMove) => do_move = true,
            Ok(BotMsg::PrepareNextMove) => {}
        }

        if do_move && bot.min_thinking_reached() {
            if let Some((mv, info)) = bot.next_move() {
                do_move = false;
                if send.send(BotResult::Move(mv)).is_err() {
                    return
                }
                if send.send(BotResult::Info(info)).is_err() {
                    return
                }
            }
        }

        can_think = bot.think();
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Info {
    pub nodes: usize,
    pub depth: usize,
    pub evaluation: i32,
    pub plan: Vec<(Placement, LockResult)>
}
