use ggez::event::EventHandler;
use ggez::{ Context, GameResult };
use ggez::timer;
use ggez::graphics;
use crate::interface::{ Gui, setup_graphics, text };
use crate::Resources;
use battle::{ Battle, Controller, Replay };
use std::collections::VecDeque;

pub struct ReplayGame<'a, P> {
    gui: Gui,
    battle: Battle,
    updates: VecDeque<(Controller, Controller)>,
    start_delay: u32,
    resources: &'a mut Resources,
    file: P
}

impl<'a, P: AsRef<std::path::Path> + Clone> ReplayGame<'a, P> {
    pub fn new(resources: &'a mut Resources, file: P) -> Self {
        let replay: Replay = serde_json::from_reader(
            std::io::BufReader::new(std::fs::File::open(file.clone()).unwrap())
        ).unwrap();
        let battle = Battle::new(
            replay.config, replay.p1_seed, replay.p2_seed, replay.garbage_seed
        );
        ReplayGame {
            gui: Gui::new(&battle, replay.p1_name, replay.p2_name),
            battle,
            updates: replay.updates,
            start_delay: 180,
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
                    self.gui.update(update, self.resources)?;
                } else {
                    let replay: Replay;
                    loop {
                        match std::fs::File::open(self.file.clone()) {
                            Ok(f) => {
                                match serde_json::from_reader(std::io::BufReader::new(f)) {
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
                    let battle = Battle::new(
                        replay.config, replay.p1_seed, replay.p2_seed, replay.garbage_seed
                    );
                    self.gui = Gui::new(&battle, replay.p1_name, replay.p2_name);
                    self.battle = battle;
                    self.updates = replay.updates;
                    self.start_delay = 180;
                }
            } else {
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