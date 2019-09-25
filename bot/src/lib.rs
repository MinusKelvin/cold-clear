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
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: crate::moves::MovementMode::ZeroG,
            use_hold: true,
            speculate: true,
            min_nodes: 0,
            max_nodes: std::usize::MAX
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

    /// Returns true if all possible piece placement sequences result in death, or some kind of
    /// error occured that crashed the bot thread.
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
    /// will only be able to provide a move after the current piece spawns and you provide the new
    /// piece information to the bot using `add_next_piece`.
    /// 
    /// It is recommended that you wait to call this function until after the current piece spawns
    /// and you update the queue using `add_next_piece`, as this will allow speculation to be
    /// resolved and at least one thinking cycle to run.
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
    /// If speculation is enabled, the piece must be in the bag. For example, if you start a new
    /// game with starting sequence IJOZT, the first time you call this function you can only
    /// provide either an L or an S piece.
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
    /// Note: combo is not the same as the displayed combo in guideline games. Here, it is better
    /// thought of as the number of pieces that have been placed that cleared lines in a row. So,
    /// generally speaking, if you break your combo, use 0 here; if you just clear a line, use 1
    /// here; and if "x Combo" appears on the screen, use x+1 here.
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

fn run(
    recv: Receiver<BotMsg>,
    send: Sender<BotResult>,
    board: Board,
    mut evaluator: impl Evaluator,
    options: Options
) {
    let mut tree = Tree::new(
        board,
        &Default::default(),
        0,
        &mut evaluator
    );

    let mut do_move = false;
    let mut cycles = 0;
    loop {
        let result = if tree.child_nodes < options.max_nodes {
            recv.try_recv()
        } else {
            recv.recv().map_err(|_| TryRecvError::Disconnected)
        };
        match result {
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
            Ok(BotMsg::NewPiece(piece)) => if tree.add_next_piece(piece) {
                // Only death is possible
                break
            }
            Ok(BotMsg::Reset {
                field, b2b, combo
            }) => {
                let mut board = tree.board;
                board.set_field(field);
                board.combo = combo;
                board.b2b_bonus = b2b;
                tree = Tree::new(
                    board,
                    &Default::default(),
                    0,
                    &mut evaluator
                );
                cycles = 0;
            }
            Ok(BotMsg::NextMove) => do_move = true,
            Ok(BotMsg::PrepareNextMove) => {}
        }

        if do_move && tree.child_nodes > options.min_nodes {
            let moves_considered = tree.child_nodes;
            match tree.into_best_child() {
                Ok(child) => {
                    do_move = false;
                    let mut plan = vec![(child.mv.clone(), child.lock.clone())];
                    if send.send(BotResult::Move(Move {
                        hold: child.hold,
                        inputs: child.mv.inputs.movements,
                        expected_location: child.mv.location
                    })).is_err() {
                        return
                    }
                    child.tree.get_plan(&mut plan);
                    if send.send(BotResult::Info(Info {
                        evaluation: child.tree.evaluation,
                        nodes: moves_considered,
                        depth: child.tree.depth,
                        cycles: cycles,
                        plan
                    })).is_err() {
                        return
                    }
                    tree = child.tree;
                    cycles = 0;
                }
                Err(t) => tree = t
            }
        }

        if tree.child_nodes < options.max_nodes &&
                tree.board.next_queue().count() > 0 &&
                tree.extend(options, &mut evaluator) {
            break
        }
        cycles += 1;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Info {
    pub nodes: usize,
    pub depth: usize,
    pub cycles: u32,
    pub evaluation: i32,
    pub plan: Vec<(Placement, LockResult)>
}
