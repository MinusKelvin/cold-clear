use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::path::PathBuf;

use battle::{Battle, Replay};
use game_util::text::Alignment;
use game_util::winit::event::VirtualKeyCode;
use game_util::winit::event_loop::EventLoopProxy;
use game_util::LocalExecutor;
use gilrs::Gamepad;
use libtetris::Controller;
use serde::{Deserialize, Serialize};

use crate::battle_ui::BattleUi;
use crate::res::Resources;

pub struct ReplayGame {
    ui: BattleUi,
    battle: Battle,
    file: PathBuf,
    updates: VecDeque<(Controller, Controller)>,
    p1_info_updates: VecDeque<Option<cold_clear::Info>>,
    p2_info_updates: VecDeque<Option<cold_clear::Info>>,
    start_delay: u32,
    p1_show_plan: bool,
    p2_show_plan: bool,
}

impl ReplayGame {
    pub fn new(file: impl Into<PathBuf>, p1_show_plan: bool, p2_show_plan: bool) -> Self {
        let file = file.into();
        let InfoReplay {
            replay,
            p1_info_updates,
            p2_info_updates,
        } = bincode::deserialize_from(libflate::deflate::Decoder::new(File::open(&file).unwrap()))
            .unwrap();
        let battle = Battle::new(
            replay.p1_config,
            replay.p2_config,
            replay.p1_seed,
            replay.p2_seed,
            replay.garbage_seed,
        );
        ReplayGame {
            ui: BattleUi::new(
                &battle,
                replay.p1_name,
                p1_show_plan,
                replay.p2_name,
                p2_show_plan,
            ),
            battle,
            updates: replay.updates,
            p1_info_updates,
            p2_info_updates,
            start_delay: 500,
            file,
            p1_show_plan,
            p2_show_plan,
        }
    }
}

impl crate::State for ReplayGame {
    fn update(
        &mut self,
        _el_proxy: &EventLoopProxy<Box<dyn crate::State>>,
        _executor: &LocalExecutor,
        _log: &mut crate::LogFile,
        res: &mut Resources,
        _keys: &HashSet<VirtualKeyCode>,
        _p1: Option<Gamepad>,
        _p2: Option<Gamepad>,
    ) {
        if self.start_delay == 0 {
            if let Some((p1_controller, p2_controller)) = self.updates.pop_front() {
                let update = self.battle.update(p1_controller, p2_controller);
                self.ui.update(
                    res,
                    update,
                    self.p1_info_updates.pop_front().flatten(),
                    self.p2_info_updates.pop_front().flatten(),
                );
            } else {
                let replay;
                loop {
                    match std::fs::File::open(&self.file) {
                        Ok(f) => {
                            match bincode::deserialize_from(libflate::deflate::Decoder::new(f)) {
                                Ok(r) => {
                                    replay = r;
                                    break;
                                }
                                Err(_) => {}
                            }
                        }
                        Err(_) => {}
                    }
                }
                let InfoReplay {
                    replay,
                    p1_info_updates,
                    p2_info_updates,
                } = replay;
                let battle = Battle::new(
                    replay.p1_config,
                    replay.p2_config,
                    replay.p1_seed,
                    replay.p2_seed,
                    replay.garbage_seed,
                );
                self.ui = BattleUi::new(
                    &battle,
                    replay.p1_name,
                    self.p1_show_plan,
                    replay.p2_name,
                    self.p2_show_plan,
                );
                self.battle = battle;
                self.updates = replay.updates;
                self.p1_info_updates = p1_info_updates;
                self.p2_info_updates = p2_info_updates;
                self.start_delay = 180;
            }
        } else {
            self.start_delay -= 1;
        }
    }

    fn render(&mut self, res: &mut Resources) {
        if self.start_delay != 0 {
            res.text.draw_text(
                &format!("{}", self.start_delay / 60 + 1),
                9.5,
                12.25,
                Alignment::Center,
                [0xFF; 4],
                3.0,
                0,
            );
            res.text.draw_text(
                &format!("{}", self.start_delay / 60 + 1),
                29.5,
                12.25,
                Alignment::Center,
                [0xFF; 4],
                3.0,
                0,
            );
        }
        self.ui.draw(res);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InfoReplay {
    pub replay: Replay,
    pub p1_info_updates: VecDeque<Option<cold_clear::Info>>,
    pub p2_info_updates: VecDeque<Option<cold_clear::Info>>,
}
