use libtetris::*;
use opening_book::Book;
use crate::evaluation::Evaluator;
use crate::{ Options, Info, Move, BotMsg };
use serde::{ Serialize, Deserialize };
use arrayvec::ArrayVec;

pub mod normal;
#[cfg(not(target_arch = "wasm32"))]
pub mod pcloop;

enum Mode<E: Evaluator> {
    Normal(normal::BotState<E>),
    PcLoop(pcloop::PcLooper)
}

#[cfg_attr(target_arch = "wasm32", derive(Serialize, Deserialize))]
pub(crate) enum Task {
    NormalThink(normal::Thinker),
    PcLoopSolve(pcloop::PcSolver)
}

#[derive(Serialize, Deserialize)]
pub(crate) enum TaskResult<V, R> {
    NormalThink(normal::ThinkResult<V, R>),
    PcLoopSolve(Option<ArrayVec<[FallingPiece; 10]>>)
}

pub(crate) struct ModeSwitchedBot<'a, E: Evaluator> {
    mode: Mode<E>,
    options: Options,
    board: Board,
    do_move: Option<u32>,
    book: Option<&'a Book>
}

impl<'a, E: Evaluator> ModeSwitchedBot<'a, E> {
    pub fn new(board: Board, options: Options, book: Option<&'a Book>) -> Self {
        #[cfg(target_arch = "wasm32")]
        let mode = Mode::Normal(normal::BotState::new(board.clone(), options));
        #[cfg(not(target_arch = "wasm32"))]
        let mode = if options.pcloop.is_some() &&
                board.get_row(0).is_empty() &&
                can_pc_loop(&board, options.use_hold) {
            Mode::PcLoop(pcloop::PcLooper::new(
                board.clone(), options.use_hold, options.mode, options.pcloop.unwrap()
            ))
        } else {
            Mode::Normal(normal::BotState::new(board.clone(), options))
        };
        ModeSwitchedBot {
            mode, options, board,
            do_move: None,
            book
        }
    }

    pub fn task_complete(&mut self, result: TaskResult<E::Value, E::Reward>) {
        match &mut self.mode {
            Mode::Normal(bot) => match result {
                TaskResult::NormalThink(result) => bot.finish_thinking(result),
                _ => {}
            }
            Mode::PcLoop(bot) => match result {
                TaskResult::PcLoopSolve(result) => bot.solution(result),
                _ => {}
            }
        }
    }

    pub fn message(&mut self, msg: BotMsg) {
        match msg {
            BotMsg::Reset { field, b2b, combo } => {
                self.board.set_field(field);
                self.board.b2b_bonus = b2b;
                self.board.combo = combo;
                match &mut self.mode {
                    Mode::Normal(bot) => bot.reset(field, b2b, combo),
                    Mode::PcLoop(_) => self.mode = Mode::Normal(
                        normal::BotState::new(self.board.clone(), self.options)
                    )
                }
            }
            BotMsg::NewPiece(piece) => {
                self.board.add_next_piece(piece);
                match &mut self.mode {
                    Mode::Normal(bot) => {
                        #[cfg(not(target_arch = "wasm32"))] {
                            if self.options.pcloop.is_some() && can_pc_loop(
                                &self.board, self.options.use_hold
                            ) {
                                self.mode = Mode::PcLoop(pcloop::PcLooper::new(
                                    self.board.clone(),
                                    self.options.use_hold,
                                    self.options.mode,
                                    self.options.pcloop.unwrap()
                                ));
                            } else {
                                bot.add_next_piece(piece);
                            }
                        }
                        #[cfg(target_arch = "wasm32")] {
                            bot.add_next_piece(piece);
                        }
                    },
                    Mode::PcLoop(bot) => bot.add_next_piece(piece)
                }
            }
            BotMsg::NextMove(incoming) => self.do_move = Some(incoming),
            BotMsg::ForceAnalysisLine(path) => match &mut self.mode {
                Mode::Normal(bot) => bot.force_analysis_line(path),
                _ => {}
            }
        }
    }

    pub fn think(&mut self, eval: &E, send_move: impl FnOnce(Move, Info)) -> Vec<Task> {
        let board = &mut self.board;
        let send_move = |mv: Move, info| {
            let next = board.advance_queue().unwrap();
            if mv.hold {
                if board.hold(next).is_none() {
                    board.advance_queue();
                }
            }
            board.lock_piece(mv.expected_location).perfect_clear;
            send_move(mv, info)
        };
        match &mut self.mode {
            Mode::Normal(bot) => {
                if let Some(incoming) = self.do_move {
                    if bot.next_move(eval, self.book, incoming, send_move) {
                        self.do_move = None;
                        #[cfg(not(target_arch = "wasm32"))] {
                            if self.options.pcloop.is_some() && can_pc_loop(
                                board, self.options.use_hold
                            ) {
                                self.mode = Mode::PcLoop(pcloop::PcLooper::new(
                                    board.clone(),
                                    self.options.use_hold,
                                    self.options.mode,
                                    self.options.pcloop.unwrap()
                                ));
                                fn nothing(_: Move, _: Info) {}
                                return self.think(eval, nothing);
                            }
                        }
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
            Mode::PcLoop(bot) => {
                if let Some(_) = self.do_move {
                    match bot.next_move() {
                        Ok((mv, info)) => {
                            send_move(mv, Info::PcLoop(info));
                            self.do_move = None;
                        }
                        Err(false) => {}
                        Err(true) => {
                            let mut bot = normal::BotState::new(self.board.clone(), self.options);
                            let mut thinks = vec![];
                            if let Ok(thinker) = bot.think() {
                                thinks.push(Task::NormalThink(thinker));
                            }
                            self.mode = Mode::Normal(bot);
                            return thinks;
                        }
                    }
                }

                bot.think().into_iter().map(Task::PcLoopSolve).collect()
            }
        }
    }

    pub fn is_dead(&self) -> bool {
        if let Mode::Normal(bot) = &self.mode {
            bot.is_dead()
        } else {
            false
        }
    }
}

impl Task {
    pub fn execute<E: Evaluator>(self, eval: &E) -> TaskResult<E::Value, E::Reward> {
        match self {
            Task::NormalThink(thinker) => TaskResult::NormalThink(thinker.think(eval)),
            Task::PcLoopSolve(solver) => TaskResult::PcLoopSolve(solver.solve())
        }
    }
}

fn can_pc_loop(board: &Board, hold_enabled: bool) -> bool {
    if board.get_row(0) != <u16 as Row>::EMPTY {
        return false;
    }
    let pieces = board.next_queue().count();
    if hold_enabled {
        let pieces = pieces + board.hold_piece.is_some() as usize;
        pieces >= 11
    } else {
        pieces >= 10
    }
}

#[cfg(target_arch = "wasm32")]
/// dummy wasm32 types because pcf can't really work on web until wasm threads come out
pub mod pcloop {
    use serde::{ Serialize, Deserialize };
    use crate::Move;
    use libtetris::{ Piece, FallingPiece, LockResult };
    use arrayvec::ArrayVec;

    #[derive(Serialize, Deserialize)]
    pub struct PcSolver;
    #[derive(Serialize, Deserialize)]
    pub struct PcLooper;

    impl PcLooper {
        pub fn add_next_piece(&mut self, _: Piece) { unreachable!() }
        pub fn think(&mut self) -> Option<PcSolver> { unreachable!() }
        pub fn next_move(&mut self) -> Result<(Move, Info), bool> { unreachable!() }
        pub fn solution(&mut self, _: Option<ArrayVec<[FallingPiece; 10]>>) { unreachable!() }
    }

    impl PcSolver {
        pub fn solve(&self) -> Option<ArrayVec<[FallingPiece; 10]>> { unreachable!() }
    }

    #[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
    pub struct Info {
        pub plan: Vec<(FallingPiece, LockResult)>
    }
}