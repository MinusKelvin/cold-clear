use ggez::event::EventHandler;
use ggez::{ Context, GameResult };
use ggez::timer;
use ggez::graphics;
use crate::interface::{ Gui, setup_graphics, text };
use crate::Resources;
use battle::{ Battle, Replay };
use libtetris::Controller;
use std::collections::VecDeque;
use serde::{ Serialize, Deserialize };
use libflate::deflate;

pub struct ReplayGame<'a, P> {
    gui: Gui,
    battle: Battle,
    updates: VecDeque<(Controller, Controller)>,
    p1_info_updates: VecDeque<Option<bot::Info>>,
    p2_info_updates: VecDeque<Option<bot::Info>>,
    start_delay: u32,
    resources: &'a mut Resources,
    file: P
}

impl<'a, P: AsRef<std::path::Path> + Clone> ReplayGame<'a, P> {
    pub fn new(resources: &'a mut Resources, file: P) -> Self {
        let InfoReplay {
            replay, p1_info_updates, p2_info_updates
        } = bincode::deserialize_from(
            deflate::Decoder::new(std::fs::File::open(file.clone()).unwrap())
        ).unwrap();
        let battle = Battle::new(
            replay.p1_config, replay.p2_config,
            replay.p1_seed, replay.p2_seed,
            replay.garbage_seed
        );
        ReplayGame {
            gui: Gui::new(&battle, replay.p1_name, replay.p2_name),
            battle,
            updates: replay.updates,
            p1_info_updates: p1_info_updates,
            p2_info_updates: p2_info_updates,
            start_delay: 500,
            resources,
            file
        }
    }
}

impl<P: AsRef<std::path::Path> + Clone> EventHandler for ReplayGame<'_, P> {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        while timer::check_update_time(ctx, 60) {
            if self.start_delay == 0 {
                if let Some(
                    (p1_controller, p2_controller)
                ) = self.updates.pop_front() {
                    let update = self.battle.update(p1_controller, p2_controller);
                    self.gui.update(
                        update,
                        self.p1_info_updates.pop_front().and_then(|x| x),
                        self.p2_info_updates.pop_front().and_then(|x| x),
                        self.resources
                    )?;
                } else {
                    let replay;
                    loop {
                        match std::fs::File::open(self.file.clone()) {
                            Ok(f) => {
                                match bincode::deserialize_from(deflate::Decoder::new(f)) {
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
                    self.gui = Gui::new(&battle, replay.p1_name, replay.p2_name);
                    self.battle = battle;
                    self.updates = replay.updates;
                    self.p1_info_updates = p1_info_updates;
                    self.p2_info_updates = p2_info_updates;
                    self.start_delay = 180;
                }
            } else {
                if self.start_delay == 180 {
                    while timer::check_update_time(ctx, 60) {}
                }
                self.start_delay -= 1;
            }
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let (scale, center) = setup_graphics(ctx)?;

        if self.start_delay != 0 {
            let txt = text(format!("{}", self.start_delay / 60 + 1), scale * 4.0, 10.0*scale);
            graphics::queue_text(ctx, &txt, [center-14.5*scale, 9.0*scale], None);
            graphics::queue_text(ctx, &txt, [center+4.5*scale, 9.0*scale], None);
        }

        self.gui.draw(ctx, self.resources, scale, center)?;
        graphics::present(ctx)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InfoReplay {
    pub replay: Replay,
    pub p1_info_updates: VecDeque<Option<bot::Info>>,
    pub p2_info_updates: VecDeque<Option<bot::Info>>
}