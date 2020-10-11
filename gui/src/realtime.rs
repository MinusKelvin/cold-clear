use game_util::glutin::VirtualKeyCode;
use battle::{ Battle, GameConfig };
use libtetris::Board;
use std::collections::{ HashSet, VecDeque };
use std::io::prelude::*;
use rand::prelude::*;
use gilrs::Gamepad;
use crate::res::Resources;
use crate::battle_ui::BattleUi;
use crate::input::InputSource;
use crate::replay::InfoReplay;

type InputFactory = dyn Fn(Board) -> (Box<dyn InputSource>, String);

pub struct RealtimeGame {
    ui: BattleUi,
    battle: Battle,
    p1_input_factory: Box<InputFactory>,
    p2_input_factory: Box<InputFactory>,
    p1_input: Box<dyn InputSource>,
    p2_input: Box<dyn InputSource>,
    p1_wins: u32,
    p2_wins: u32,
    p1_info_updates: VecDeque<Option<cold_clear::Info>>,
    p2_info_updates: VecDeque<Option<cold_clear::Info>>,
    state: State,
    p1_config: GameConfig,
    p2_config: GameConfig,
}

enum State {
    Playing,
    GameOver(u32),
    Starting(u32)
}

impl RealtimeGame {
    pub fn new(
        p1: Box<InputFactory>,
        p2: Box<InputFactory>,
        p1_config: GameConfig,
        p2_config: GameConfig
    ) -> Self {
        let mut battle = Battle::new(
            p1_config, p2_config, thread_rng().gen(), thread_rng().gen(), thread_rng().gen()
        );
        let (p1_input, p1_name) = p1(battle.player_1.board.to_compressed());
        let (p2_input, p2_name) = p2(battle.player_2.board.to_compressed());
        battle.replay.p1_name = p1_name.clone();
        battle.replay.p2_name = p2_name.clone();
        RealtimeGame {
            ui: BattleUi::new(&battle, p1_name, p2_name),
            battle,
            p1_input_factory: p1,
            p2_input_factory: p2,
            p1_input, p2_input,
            p1_wins: 0,
            p2_wins: 0,
            p1_info_updates: VecDeque::new(),
            p2_info_updates: VecDeque::new(),
            state: State::Starting(180),
            p1_config, p2_config
        }
    }
}

impl crate::State for RealtimeGame {
    fn update(
        &mut self,
        log: &mut crate::LogFile,
        res: &mut Resources,
        keys: &HashSet<VirtualKeyCode>,
        p1: Option<Gamepad>,
        p2: Option<Gamepad>
    ) -> Option<Box<dyn crate::State>> {
        let do_update = match self.state {
            State::GameOver(0) => {
                let r: Result<(), Box<dyn std::error::Error>> = (|| {
                    let mut encoder = libflate::deflate::Encoder::new(
                        std::fs::File::create("replay.dat"
                    )?);
                    bincode::serialize_into(
                        &mut encoder,
                        &InfoReplay {
                            replay: self.battle.replay.clone(),
                            p1_info_updates: self.p1_info_updates.clone(),
                            p2_info_updates: self.p2_info_updates.clone()
                        }
                    )?;
                    encoder.finish().into_result()?;
                    Ok(())
                })();
                if let Err(e) = r {
                    writeln!(log, "Failure saving replay: {}", e).ok();
                }

                self.battle = Battle::new(
                    self.p1_config, self.p2_config,
                    thread_rng().gen(), thread_rng().gen(), thread_rng().gen()
                );

                let (p1_input, p1_name) = (self.p1_input_factory)(
                    self.battle.player_1.board.to_compressed()
                );
                let (p2_input, p2_name) = (self.p2_input_factory)(
                    self.battle.player_2.board.to_compressed()
                );

                self.ui = BattleUi::new(&self.battle, p1_name.clone(), p2_name.clone());
                self.p1_input = p1_input;
                self.p2_input = p2_input;
                self.battle.replay.p1_name = p1_name;
                self.battle.replay.p2_name = p2_name;

                self.p1_info_updates.clear();
                self.p2_info_updates.clear();

                self.state = State::Starting(180);
                false
            }
            State::GameOver(ref mut delay) => {
                *delay -= 1;
                true
            }
            State::Starting(0) => {
                self.state = State::Playing;
                true
            }
            State::Starting(ref mut delay) => {
                *delay -= 1;
                false
            }
            State::Playing => true
        };

        if do_update {
            let p1_controller = self.p1_input.controller(keys, p1);
            let p2_controller = self.p2_input.controller(keys, p2.or(p1));

            let update = self.battle.update(p1_controller, p2_controller);

            let p1_info_update = self.p1_input.update(
                &self.battle.player_1.board, &update.player_1.events,
                self.battle.player_1.garbage_queue
            );
            let p2_info_update = self.p2_input.update(
                &self.battle.player_2.board, &update.player_2.events,
                self.battle.player_2.garbage_queue
            );

            self.p1_info_updates.push_back(p1_info_update.clone());
            self.p2_info_updates.push_back(p2_info_update.clone());

            if let State::Playing = self.state {
                for event in &update.player_1.events {
                    use battle::Event::*;
                    match event {
                        GameOver => {
                            self.p2_wins += 1;
                            self.state = State::GameOver(300);
                        }
                        _ => {}
                    }
                }
                for event in &update.player_2.events {
                    use battle::Event::*;
                    match event {
                        GameOver => {
                            self.p1_wins += 1;
                            self.state = State::GameOver(300);
                        }
                        _ => {}
                    }
                }
            }

            self.ui.update(res, update, p1_info_update, p2_info_update);
        }

        None
    }

    fn render(&mut self, res: &mut Resources) {
        res.text.draw_text(
            &format!("{} - {}", self.p1_wins, self.p2_wins),
            20.0, 3.0,
            game_util::Alignment::Center,
            [0xFF; 4], 1.5, 0
        );

        if let State::Starting(timer) = self.state {
            res.text.draw_text(
                &format!("{}", timer / 60 + 1),
                9.5, 12.25,
                game_util::Alignment::Center,
                [0xFF; 4], 3.0, 0
            );
            res.text.draw_text(
                &format!("{}", timer / 60 + 1),
                29.5, 12.25,
                game_util::Alignment::Center,
                [0xFF; 4], 3.0, 0
            );
        }

        self.ui.draw(res);
    }
}