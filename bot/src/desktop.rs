use std::sync::mpsc::{ Sender, Receiver, TryRecvError, channel };
use std::sync::Arc;
use libtetris::*;
use crate::evaluation::Evaluator;
use crate::moves::Move;
use crate::{ Options, Info, AsyncBotState, BotMsg, BotPollState };

pub struct Interface {
    send: Sender<BotMsg>,
    recv: Receiver<(Move, Info)>,
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
            send, recv
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
    pub fn request_next_move(&self, incoming: u32) {
        self.send.send(BotMsg::NextMove(incoming)).ok();
    }

    /// Checks to see if the bot has provided the previously requested move yet.
    /// 
    /// The returned move contains both a path and the expected location of the placed piece. The
    /// returned path is reasonably good, but you might want to use your own pathfinder to, for
    /// example, exploit movement intricacies in the game you're playing.
    /// 
    /// If the piece couldn't be placed in the expected location, you must call `reset` to reset the
    /// game field, back-to-back status, and combo values.
    pub fn poll_next_move(&self) -> Result<(Move, Info), BotPollState> {
        self.recv.try_recv().map_err(|e| match e {
            TryRecvError::Empty => BotPollState::Waiting,
            TryRecvError::Disconnected => BotPollState::Dead
        })
    }

    /// Waits until the bot provides the previously requested move.
    /// 
    /// `None` is returned if the bot is dead.
    pub fn block_next_move(&self) -> Option<(Move, Info)> {
        self.recv.recv().ok()
    }

    /// Adds a new piece to the end of the queue.
    /// 
    /// If speculation is enabled, the piece *must* be in the bag. For example, if in the current
    /// bag you've provided the sequence IJOZT, then the next time you call this function you can
    /// only provide either an L or an S piece.
    pub fn add_next_piece(&self, piece: Piece) {
        self.send.send(BotMsg::NewPiece(piece)).ok();
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
    pub fn reset(&self, field: [[bool; 10]; 40], b2b_active: bool, combo: u32) {
        self.send.send(BotMsg::Reset {
            field, b2b: b2b_active, combo
        }).ok();
    }

    /// Specifies a line that Cold Clear should analyze before making any moves.
    pub fn force_analysis_line(&self, path: Vec<FallingPiece>) {
        self.send.send(BotMsg::ForceAnalysisLine(path)).ok();
    }
}

fn run(
    recv: Receiver<BotMsg>,
    send: Sender<(Move, Info)>,
    mut board: Board,
    eval: impl Evaluator + 'static,
    options: Options
) {
    if options.threads == 0 {
        panic!("Invalid number of threads: 0");
    }

    while board.next_queue().next().is_none() {
        match recv.recv() {
            Err(_) => return,
            Ok(BotMsg::NewPiece(piece)) => board.add_next_piece(piece),
            Ok(BotMsg::Reset { field, b2b, combo }) =>{
                board.set_field(field);
                board.combo = combo;
                board.b2b_bonus = b2b;
            }
            Ok(BotMsg::NextMove(_)) => {}
            Ok(BotMsg::ForceAnalysisLine(_)) => {}
        }
    }

    let mut bot = AsyncBotState::new(board, options);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(options.threads as usize)
        .build().unwrap();

    let (result_send, result_recv) = channel();

    let eval = Arc::new(eval);
    while !bot.is_dead() {
        let (new_thinks, can_think) = bot.think(
            &eval,
            |mv, info| { send.send((mv, info)).ok(); }
        );
        for thinker in new_thinks {
            let result_send = result_send.clone();
            let eval = eval.clone();
            pool.spawn_fifo(move || {
                result_send.send(thinker.think(&eval)).ok();
            });
        }

        for result in result_recv.try_iter() {
            bot.think_done(result);
        }

        let result = if can_think {
            recv.try_recv()
        } else {
            recv.recv().map_err(|_| TryRecvError::Disconnected)
        };
        match result {
            Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
            Ok(msg) => bot.message(msg)
        }

        if can_think && bot.should_wait_for_think() {
            if let Ok(result) = result_recv.recv() {
                bot.think_done(result);
            }
        }
    }
}