use std::sync::mpsc::{ Sender, Receiver, TryRecvError, channel };
use std::sync::Arc;
use libtetris::*;
use crate::evaluation::Evaluator;
use crate::moves::Move;
use crate::{ Options, Info, BotState };

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

    /// Specifies a line that Cold Clear should analyze before making any moves.
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

    let mut bot = BotState::new(board, options, Arc::new(evaluator));

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(options.threads as usize)
        .build().unwrap();

    let (result_send, result_recv) = channel();
    let mut tasks = 0;
    let mut can_think = true;

    while !bot.is_dead() {
        let result = if can_think {
            recv.try_recv()
        } else {
            recv.recv().map_err(|_| TryRecvError::Disconnected)
        };
        match result {
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
            match bot.think() {
                Ok(thinker) => {
                    let result_send = result_send.clone();
                    pool.spawn_fifo(move || {
                        result_send.send(thinker.think()).ok();
                    });
                    tasks += 1;
                    can_think = true;
                }
                Err(could_think) => can_think = could_think || tasks != 0
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