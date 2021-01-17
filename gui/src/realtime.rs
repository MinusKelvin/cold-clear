use game_util::text::Alignment;
use game_util::winit::event::VirtualKeyCode;
use game_util::winit::event_loop::EventLoopProxy;
use game_util::LocalExecutor;
use battle::Battle;
use std::collections::{ HashSet, VecDeque };
use std::io::prelude::*;
use rand::prelude::*;
use gilrs::Gamepad;
use crate::Options;
use crate::res::Resources;
use crate::battle_ui::BattleUi;
use crate::input::InputSource;
use crate::replay::InfoReplay;

pub struct RealtimeGame {
    ui: BattleUi,
    battle: Battle,
    options: Option<Options>,
    p1_input: Box<dyn InputSource>,
    p2_input: Box<dyn InputSource>,
    p1_wins: u32,
    p2_wins: u32,
    p1_info_updates: VecDeque<Option<cold_clear::Info>>,
    p2_info_updates: VecDeque<Option<cold_clear::Info>>,
    state: State,
}

enum State {
    Playing,
    GameOver(u32),
    Starting(u32)
}

impl RealtimeGame {
    pub(super) async fn new(
        options: Options,
        p1_wins: u32,
        p2_wins: u32
    ) -> Self {
        let mut battle = Battle::new(
            options.p1.game, options.p2.game,
            thread_rng().gen(), thread_rng().gen(), thread_rng().gen()
        );
        let (p1_input, p1_name) = options.p1.to_player(
            battle.player_1.board.to_compressed()
        ).await;
        let (p2_input, p2_name) = options.p2.to_player(
            battle.player_2.board.to_compressed()
        ).await;
        battle.replay.p1_name = p1_name.clone();
        battle.replay.p2_name = p2_name.clone();
        RealtimeGame {
            ui: BattleUi::new(&battle, p1_name, p2_name),
            battle,
            options: Some(options),
            p1_input, p2_input,
            p1_wins, p2_wins,
            p1_info_updates: VecDeque::new(),
            p2_info_updates: VecDeque::new(),
            state: State::Starting(180),
        }
    }
}

impl crate::State for RealtimeGame {
    fn update(
        &mut self,
        el_proxy: &EventLoopProxy<Box<dyn crate::State>>,
        executor: &LocalExecutor,
        log: &mut crate::LogFile,
        res: &mut Resources,
        keys: &HashSet<VirtualKeyCode>,
        p1: Option<Gamepad>,
        p2: Option<Gamepad>
    ) {
        let do_update = match self.state {
            State::GameOver(0) => if let Some(options) = self.options.take() {
                let r: Result<(), Box<dyn std::error::Error>> = (|| {
                    let mut encoder = libflate::deflate::Encoder::new(
                        std::fs::File::create("replay.dat")?
                    );
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

                let p1_wins = self.p1_wins;
                let p2_wins = self.p2_wins;
                let el_proxy = el_proxy.clone();
                executor.spawn(async move {
                    let next_state = RealtimeGame::new(options, p1_wins, p2_wins).await;
                    el_proxy.send_event(Box::new(next_state)).ok();
                });
                false
            } else {
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
    }

    fn render(&mut self, res: &mut Resources) {
        res.text.draw_text(
            &format!("{} - {}", self.p1_wins, self.p2_wins),
            20.0, 3.0,
            Alignment::Center,
            [0xFF; 4], 1.5, 0
        );

        if let State::Starting(timer) = self.state {
            res.text.draw_text(
                &format!("{}", timer / 60 + 1),
                9.5, 12.25,
                Alignment::Center,
                [0xFF; 4], 3.0, 0
            );
            res.text.draw_text(
                &format!("{}", timer / 60 + 1),
                29.5, 12.25,
                Alignment::Center,
                [0xFF; 4], 3.0, 0
            );
        }

        self.ui.draw(res);
    }
}