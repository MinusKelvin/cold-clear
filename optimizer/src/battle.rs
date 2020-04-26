use libtetris::{ Board, ColoredRow, FallingPiece, Controller };
use battle::{ Battle, Replay, Event, PieceMoveExecutor, GameConfig };
use cold_clear::evaluation::Evaluator;
use rand::prelude::*;
use serde::{ Serialize, Deserialize };
use std::collections::VecDeque;

pub struct BotInput<E: Evaluator> {
    pub controller: Controller,
    executing: Option<(FallingPiece, PieceMoveExecutor)>,
    bot: cold_clear::BotState<E>,
    eval: E
}

const THINK_AMOUNT: usize = 10;

impl<E: Evaluator> BotInput<E> {
    pub fn new(board: Board, eval: E) -> Self {
        let mut this = BotInput {
            controller: Controller::default(),
            executing: None,
            bot: cold_clear::BotState::new(board, Default::default()),
            eval
        };
        for _ in 0..180 {
            // equivalent of 3 realtime seconds of thinking
            this.think();
        }
        this
    }

    fn think(&mut self) {
        for _ in 0..THINK_AMOUNT {
            match self.bot.think() {
                Ok(thinker) => {
                    self.bot.finish_thinking(thinker.think(&self.eval));
                }
                Err(_) => {
                    // can't think anymore
                    break
                }
            }
        }
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
                        self.bot.next_move(&self.eval, incoming, |mv, inf| {
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

pub fn do_battle(
    p1: impl Evaluator + Clone, p2: impl Evaluator + Clone
) -> Option<(InfoReplay, bool)> {
    let mut battle = Battle::new(
        GameConfig::fast_config(), GameConfig::fast_config(),
        thread_rng().gen(), thread_rng().gen(), thread_rng().gen()
    );

    battle.replay.p1_name = format!("Cold Clear\n{}", p1.name());
    battle.replay.p2_name = format!("Cold Clear\n{}", p2.name());

    let mut p1 = BotInput::new(battle.player_1.board.to_compressed(), p1);
    let mut p2 = BotInput::new(battle.player_2.board.to_compressed(), p2);

    let mut p1_info_updates = VecDeque::new();
    let mut p2_info_updates = VecDeque::new();

    let p1_won;
    'battle: loop {
        let update = battle.update(p1.controller, p2.controller);
        p1_info_updates.push_back(p1.update(
            &battle.player_1.board,
            &update.player_1.events,
            battle.player_1.garbage_queue
        ));
        p2_info_updates.push_back(p2.update(
            &battle.player_2.board,
            &update.player_2.events,
            battle.player_2.garbage_queue
        ));

        for event in &update.player_1.events {
            use battle::Event::*;
            match event {
                GameOver => {
                    p1_won = false;
                    break 'battle;
                }
                _ => {}
            }
        }
        for event in &update.player_2.events {
            use battle::Event::*;
            match event {
                GameOver => {
                    p1_won = true;
                    break 'battle;
                }
                _ => {}
            }
        }

        if battle.replay.updates.len() > 54000 { // 15 minutes
            return None
        }
    }

    for _ in 0..180 {
        battle.replay.updates.push_back(Default::default());
        p1_info_updates.push_back(None);
        p2_info_updates.push_back(None);
    }

    Some((InfoReplay {
        replay: battle.replay,
        p1_info_updates,
        p2_info_updates
    }, p1_won))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InfoReplay {
    pub replay: Replay,
    pub p1_info_updates: VecDeque<Option<cold_clear::Info>>,
    pub p2_info_updates: VecDeque<Option<cold_clear::Info>>
}