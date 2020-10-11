use battle::Replay;
use serde::{ Serialize, Deserialize };
use std::collections::{ HashSet, VecDeque };
use std::path::PathBuf;
use std::fs::File;
use battle::Battle;
use libtetris::Controller;
use gilrs::Gamepad;
use game_util::glutin::VirtualKeyCode;
use crate::battle_ui::BattleUi;
use crate::res::Resources;

pub struct ReplayGame {
    ui: BattleUi,
    battle: Battle,
    file: PathBuf,
    updates: VecDeque<(Controller, Controller)>,
    p1_info_updates: VecDeque<Option<cold_clear::Info>>,
    p2_info_updates: VecDeque<Option<cold_clear::Info>>,
    start_delay: u32
}

impl ReplayGame {
    pub fn new(file: impl Into<PathBuf>) -> Self {
        let file = file.into();
        let InfoReplay {
            replay, p1_info_updates, p2_info_updates
        } = bincode::deserialize_from(
            libflate::deflate::Decoder::new(File::open(&file).unwrap())
        ).unwrap();
        let battle = Battle::new(
            replay.p1_config, replay.p2_config,
            replay.p1_seed, replay.p2_seed,
            replay.garbage_seed
        );
        ReplayGame {
            ui: BattleUi::new(&battle, replay.p1_name, replay.p2_name),
            battle,
            updates: replay.updates,
            p1_info_updates, p2_info_updates,
            start_delay: 500,
            file
        }
    }
}

impl crate::State for ReplayGame {
    fn update(
        &mut self,
        _log: &mut crate::LogFile,
        res: &mut Resources,
        _keys: &HashSet<VirtualKeyCode>,
        _p1: Option<Gamepad>,
        _p2: Option<Gamepad>
    ) -> Option<Box<dyn crate::State>> {
        if self.start_delay == 0 {
            if let Some((p1_controller, p2_controller)) = self.updates.pop_front() {
                let update = self.battle.update(p1_controller, p2_controller);
                self.ui.update(
                    res, update,
                    self.p1_info_updates.pop_front().flatten(),
                    self.p2_info_updates.pop_front().flatten()
                );
            } else {
                let replay;
                loop {
                    match std::fs::File::open(&self.file) {
                        Ok(f) => {
                            match bincode::deserialize_from(libflate::deflate::Decoder::new(f)) {
                                Ok(r) => {
                                    replay = r;
                                    break
                                }
                                Err(_) => {}
                            }
                        }
                        Err(_) => {}
                    }
                }
                let InfoReplay { replay, p1_info_updates, p2_info_updates } = replay;
                let battle = Battle::new(
                    replay.p1_config, replay.p2_config,
                    replay.p1_seed, replay.p2_seed,
                    replay.garbage_seed
                );
                self.ui = BattleUi::new(&battle, replay.p1_name, replay.p2_name);
                self.battle = battle;
                self.updates = replay.updates;
                self.p1_info_updates = p1_info_updates;
                self.p2_info_updates = p2_info_updates;
                self.start_delay = 180;
            }
        } else {
            self.start_delay -= 1;
        }
        None
    }

    fn render(&mut self, res: &mut Resources) {
        if self.start_delay != 0 {
            res.text.draw_text(
                &format!("{}", self.start_delay / 60 + 1),
                9.5, 12.25,
                game_util::Alignment::Center,
                [0xFF; 4], 3.0, 0
            );
            res.text.draw_text(
                &format!("{}", self.start_delay / 60 + 1),
                29.5, 12.25,
                game_util::Alignment::Center,
                [0xFF; 4], 3.0, 0
            );
        }
        self.ui.draw(res);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InfoReplay {
    pub replay: Replay,
    pub p1_info_updates: VecDeque<Option<cold_clear::Info>>,
    pub p2_info_updates: VecDeque<Option<cold_clear::Info>>
}