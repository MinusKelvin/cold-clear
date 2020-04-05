use libtetris::{ Board, ColoredRow, FallingPiece, Controller };
use battle::{ Event, PieceMoveExecutor };
use std::time::{ Instant, Duration };
use cold_clear::evaluation::Evaluator;

pub struct BotInput<E: Evaluator> {
    pub controller: Controller,
    executing: Option<(FallingPiece, PieceMoveExecutor)>,
    time_budget: Duration,
    bot: cold_clear::BotState<E>
}

const THINK_AMOUNT: Duration = Duration::from_millis(4);

impl<E: Evaluator> BotInput<E> {
    pub fn new(board: Board, eval: E) -> Self {
        let mut this = BotInput {
            controller: Controller::default(),
            executing: None,
            time_budget: Duration::new(0, 0),
            bot: cold_clear::BotState::new(board, Default::default(), eval)
        };
        for _ in 0..180 {
            // equivalent of 3 realtime seconds of thinking
            this.think();
        }
        this
    }

    fn think(&mut self) {
        std::thread::yield_now(); // get a new timeslice
        while self.time_budget < THINK_AMOUNT {
            let start = Instant::now();
            match self.bot.think() {
                Ok(thinker) => {
                    self.bot.finish_thinking(thinker.think());
                }
                Err(_) => {
                    // can't think anymore
                    self.time_budget = THINK_AMOUNT;
                    break
                }
            }
            self.time_budget += start.elapsed();
        }
        self.time_budget -= THINK_AMOUNT;
    }

    pub fn update(
        &mut self, board: &Board<ColoredRow>, events: &[Event], incoming: u32
    ) -> Option<cold_clear::Info> {
        self.think();

        let mut info = None;
        for event in events {
            match event {
                Event::PieceSpawned { new_in_queue } => {
                    self.bot.add_next_piece(*new_in_queue);
                    if self.executing.is_none() {
                        let exec = &mut self.executing;
                        self.bot.next_move(incoming, |mv, inf| {
                            info = Some(inf);
                            *exec = Some((
                                mv.expected_location,
                                PieceMoveExecutor::new(mv.hold, mv.inputs.into_iter().collect(), 0)
                            ));
                        });
                    }
                }
                Event::GarbageAdded(_) => {
                    self.bot.reset(board.get_field(), board.b2b_bonus, board.combo);
                }
                _ => {}
            }
        }

        if let Some((expected, ref mut executor)) = self.executing {
            if let Some(loc) = executor.update(&mut self.controller, board, events) {
                if loc != expected {
                    self.bot.reset(board.get_field(), board.b2b_bonus, board.combo);
                }
                self.executing = None;
            }
        }
        info
    }
}