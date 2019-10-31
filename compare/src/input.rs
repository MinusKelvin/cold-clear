use libtetris::{ Board, ColoredRow, FallingPiece, LockResult };
use battle::{ Controller, Event, PieceMoveExecutor };
use std::time::{ Instant, Duration };
use bot::evaluation::Evaluator;
use bot::moves::Placement;

pub struct BotInput<E: Evaluator> {
    pub controller: Controller,
    executing: Option<(FallingPiece, PieceMoveExecutor)>,
    time_budget: Duration,
    bot: bot::BotState<E>,
    opponent_time_budget: Duration,
    opponent_bot: bot::BotState<E>,
    opponent_plan: Option<Vec<(Placement, LockResult)>>,
    opponent_cycles: u32
}

const THINK_AMOUNT: Duration = Duration::from_millis(5);

impl<E: Evaluator + Clone> BotInput<E> {
    pub fn new(board: Board, opponent_board: Board, eval: E) -> Self {
        let mut this = BotInput {
            controller: Controller::default(),
            executing: None,
            time_budget: Duration::new(0, 0),
            bot: bot::BotState::new(board, Default::default(), eval.clone()),
            opponent_bot: bot::BotState::new(opponent_board, Default::default(), eval),
            opponent_time_budget: Duration::new(0, 0),
            opponent_plan: None,
            opponent_cycles: 0
        };
        for _ in 0..180 {
            // equivalent of 3 realtime seconds of thinking
            this.think(false);
        }
        this
    }

    fn think(&mut self, opponent_think: bool) {
        std::thread::yield_now(); // get a new timeslice
        while self.time_budget < THINK_AMOUNT {
            let start = Instant::now();
            if !self.bot.think() {
                // can't think anymore
                self.time_budget = THINK_AMOUNT;
                break
            }
            self.time_budget += start.elapsed();
        }
        self.time_budget -= THINK_AMOUNT;
        if opponent_think {
            while self.opponent_time_budget < THINK_AMOUNT {
                let start = Instant::now();
                if !self.opponent_bot.think() {
                    // can't think anymore
                    self.opponent_time_budget = THINK_AMOUNT;
                    break
                }
                self.opponent_cycles += 1;
                if self.opponent_cycles == 100 {
                    self.opponent_plan = self.opponent_bot.peek_next_move(None)
                        .map(|(_, info)| info.plan);
                    self.opponent_cycles = 0;
                }
                self.opponent_time_budget += start.elapsed();
            }
            self.opponent_time_budget -= THINK_AMOUNT;
        }
    }

    pub fn update(
        &mut self,
        board: &Board<ColoredRow>,
        opponent: Option<(&Board<ColoredRow>, &[Event])>,
        events: &[Event],
        garbage_queue: u32
    ) -> Option<bot::Info> {
        self.think(opponent.is_some());

        let mut info = None;
        for event in events {
            match event {
                Event::PieceSpawned { new_in_queue } => {
                    self.bot.add_next_piece(*new_in_queue);
                    if self.executing.is_none() {
                        if let Some((mv, inf)) = self.bot.next_move(
                            self.opponent_plan.as_ref().map(|x| &**x)
                        ) {
                            info = Some(inf);
                            self.executing = Some((
                                mv.expected_location,
                                PieceMoveExecutor::new(
                                    mv.hold, mv.stall_for, mv.inputs.into_iter().collect()
                                )
                            ));
                        }
                    }
                }
                Event::GarbageAdded(_) => {
                    self.bot.reset(board.get_field(), board.b2b_bonus, board.combo);
                }
                _ => {}
            }
        }

        if let Some((opponent_board, opponent_events)) = opponent {
            for event in opponent_events {
                match event {
                    Event::PieceSpawned { new_in_queue } =>
                        self.opponent_bot.add_next_piece(*new_in_queue),
                    Event::GarbageAdded(_) => self.opponent_bot.reset(
                        opponent_board.get_field(), opponent_board.b2b_bonus, opponent_board.combo
                    ),
                    Event::PiecePlaced { piece, .. } => {
                        self.opponent_bot.do_move(*piece);
                        self.opponent_cycles = 0;
                    },
                    _ => {}
                }
            }
        }

        if let Some((expected, ref mut executor)) = self.executing {
            if let Some(loc) = executor.update(&mut self.controller, board, events, garbage_queue) {
                if loc != expected {
                    self.bot.reset(board.get_field(), board.b2b_bonus, board.combo);
                }
                self.executing = None;
            }
        }
        info
    }
}