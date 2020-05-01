use libtetris::*;
use crate::evaluation::Evaluator;
use crate::{ Options, Info, Move, BotMsg };
use serde::{ Serialize, Deserialize };

pub mod normal;

enum Mode<E: Evaluator> {
    Normal(normal::BotState<E>),
}

#[derive(Serialize, Deserialize)]
pub(crate) enum Task {
    NormalThink(normal::Thinker),
}

#[derive(Serialize, Deserialize)]
pub(crate) enum TaskResult<V, R> {
    NormalThink(normal::ThinkResult<V, R>)
}

pub(crate) struct ModeSwitchedBot<E: Evaluator> {
    mode: Mode<E>,
    options: Options,
    do_move: Option<u32>
}

impl<E: Evaluator> ModeSwitchedBot<E> {
    pub fn new(board: Board, options: Options) -> Self {
        ModeSwitchedBot {
            mode: Mode::Normal(normal::BotState::new(board, options)),
            options,
            do_move: None
        }
    }

    pub fn task_complete(&mut self, result: TaskResult<E::Value, E::Reward>) {
        match &mut self.mode {
            Mode::Normal(bot) => match result {
                TaskResult::NormalThink(result) => bot.finish_thinking(result)
            }
        }
    }

    pub fn message(&mut self, msg: BotMsg) {
        match msg {
            BotMsg::Reset { field, b2b, combo } => match &mut self.mode {
                Mode::Normal(bot) => bot.reset(field, b2b, combo),
            },
            BotMsg::NewPiece(piece) => match &mut self.mode {
                Mode::Normal(bot) => bot.add_next_piece(piece),
            },
            BotMsg::NextMove(incoming) => self.do_move = Some(incoming),
            BotMsg::ForceAnalysisLine(path) => match &mut self.mode {
                Mode::Normal(bot) => bot.force_analysis_line(path)
            }
        }
    }

    pub fn think(&mut self, eval: &E, send_move: impl FnOnce(Move, Info)) -> Vec<Task> {
        match &mut self.mode {
            Mode::Normal(bot) => {
                if let Some(incoming) = self.do_move {
                    if bot.next_move(eval, incoming, send_move) {
                        self.do_move = None;
                    }
                }

                let mut thinks = vec![];
                for _ in 0..10 {
                    if bot.outstanding_thinks >= self.options.threads {
                        return thinks
                    }
                    match bot.think() {
                        Ok(thinker) => {
                            thinks.push(Task::NormalThink(thinker));
                        }
                        Err(false) => return thinks,
                        Err(true) => {}
                    }
                }
                thinks
            }
        }
    }
}

impl Task {
    pub fn execute<E: Evaluator>(self, eval: &E) -> TaskResult<E::Value, E::Reward> {
        match self {
            Task::NormalThink(thinker) => TaskResult::NormalThink(thinker.think(eval))
        }
    }
}