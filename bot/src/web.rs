use webutil::prelude::*;
use webutil::worker::{ Worker, WorkerSender };
use webutil::channel::{ channel, Receiver };
use serde::{ Serialize, de::DeserializeOwned };
use libtetris::*;
use crate::evaluation::Evaluator;
use crate::moves::Move;
use crate::{ Options, Info, BotMsg, BotPollState };
use crate::modes::{ ModeSwitchedBot, Task, TaskResult };
use futures_util::{ select, pin_mut };
use futures_util::FutureExt;

// trait aliases (#41517) would make my life SOOOOO much easier
// pub trait WebCompatibleEvaluator = where
//     Self: Evaluator + Clone + Serialize + DeserializeOwned + 'static,
//     <Self as Evaluator>::Reward: Serialize + DeserializeOwned,
//     <Self as Evaluator>::Value: Serialize + DeserializeOwned;

pub struct Interface(Option<Worker<BotMsg, Option<(Move, Info)>>>);

impl Interface {
    /// Launches a bot worker with the specified starting board and options.
    pub async fn launch<E>(
        board: Board,
        options: Options,
        evaluator: E
    ) -> Self
    where
        E: Evaluator + Clone + Serialize + DeserializeOwned + 'static,
        E::Value: Serialize + DeserializeOwned,
        E::Reward: Serialize + DeserializeOwned
    {
        if options.threads == 0 {
            panic!("Invalid number of threads: 0");
        }

        let worker = Worker::new(bot_thread, &(board, options, evaluator)).await.unwrap();

        Interface(Some(worker))
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
        if let Some(worker) = &self.0 {
            worker.send(&BotMsg::NextMove(incoming)).ok().unwrap();
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
    pub fn poll_next_move(&mut self) -> Result<(Move, Info), BotPollState> {
        match &self.0 {
            Some(worker) => match worker.try_recv() {
                Some(Some(mv)) => Ok(mv),
                Some(None) => {
                    self.0 = None;
                    Err(BotPollState::Dead)
                }
                None => Err(BotPollState::Waiting)
            }
            None => Err(BotPollState::Dead)
        }
    }

    /// Waits for the bot to provide the previously requested move.
    /// 
    /// `None` is returned if the bot is dead.
    pub async fn next_move(&mut self) -> Option<(Move, Info)> {
        match self.0.as_ref()?.recv().await {
            Some(v) => Some(v),
            None => {
                self.0 = None;
                None
            }
        }
    }

    /// Adds a new piece to the end of the queue.
    /// 
    /// If speculation is enabled, the piece *must* be in the bag. For example, if in the current
    /// bag you've provided the sequence IJOZT, then the next time you call this function you can
    /// only provide either an L or an S piece.
    pub fn add_next_piece(&self, piece: Piece) {
        if let Some(worker) = &self.0 {
            worker.send(&BotMsg::NewPiece(piece)).unwrap();
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
    pub fn reset(&self, field: [[bool; 10]; 40], b2b_active: bool, combo: u32) {
        if let Some(worker) = &self.0 {
            worker.send(&BotMsg::Reset {
                field, b2b: b2b_active, combo
            }).unwrap();
        }
    }

    /// Specifies a line that Cold Clear should analyze before making any moves.
    pub fn force_analysis_line(&self, path: Vec<FallingPiece>) {
        if let Some(worker) = &self.0 {
            worker.send(&BotMsg::ForceAnalysisLine(path)).unwrap();
        }
    }
}

fn bot_thread<E>(
    (board, options, eval): (Board, Options, E),
    recv: Receiver<BotMsg>,
    send: WorkerSender<Option<(Move, Info)>>
) where
    E: Evaluator + Clone + Serialize + DeserializeOwned + 'static,
    E::Value: Serialize + DeserializeOwned,
    E::Reward: Serialize + DeserializeOwned
{
    spawn_local(async move {
        let (result_send, result_recv) = channel::<TaskResult<E::Value, E::Reward>>();
        let (task_send, task_recv) = channel::<Task>();
        // spawn workers
        for _ in 0..options.threads {
            let result_send = result_send.clone();
            let task_recv = task_recv.clone();
            let eval = eval.clone();
            spawn_local(async move {
                let worker = Worker::new(worker, &eval).await.unwrap();
                while let Some(task) = task_recv.recv().await {
                    worker.send(&task).unwrap();
                    result_send.send(worker.recv().await).ok().unwrap();
                }
            });
        }

        let mut state = ModeSwitchedBot::new(board, options);

        loop {
            let new_tasks = state.think(&eval, |mv, info| send.send(&Some((mv, info))));
            for task in new_tasks {
                task_send.send(task).ok().unwrap();
            }

            let msg = recv.recv().fuse();
            let task = result_recv.recv().fuse();
            pin_mut!(msg, task);
            select! {
                msg = msg => match msg {
                    Some(msg) => state.message(msg),
                    None => break
                },
                task = task => state.task_complete(task.unwrap())
            }

            if state.is_dead() {
                break
            }
        }

        send.send(&None);
    });
}

fn worker<E>(eval: E, recv: Receiver<Task>, send: WorkerSender<TaskResult<E::Value, E::Reward>>)
where
    E: Evaluator + Serialize + DeserializeOwned + 'static,
    E::Value: Serialize + DeserializeOwned,
    E::Reward: Serialize + DeserializeOwned
{
    spawn_local(async move {
        while let Some(v) = recv.recv().await {
            send.send(&v.execute(&eval));
        }
    })
}