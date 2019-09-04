use ggez::event::EventHandler;
use ggez::{ Context, GameResult };
use ggez::timer;
use ggez::graphics;
use crate::interface::{ Gui, setup_graphics, text };
use crate::Resources;
use libtetris::{ Battle, Controller, Info, Replay };
use std::collections::VecDeque;

pub struct ReplayGame<'a> {
    gui: Gui,
    battle: Battle,
    updates: VecDeque<(Controller, Option<Info>, Controller, Option<Info>)>,
    start_delay: u32,
    resources: &'a mut Resources
}

impl<'a> ReplayGame<'a> {
    pub fn new(resources: &'a mut Resources, file: impl AsRef<std::path::Path>) -> Self {
        let replay: Replay = serde_json::from_reader(
            std::io::BufReader::new(std::fs::File::open(file).unwrap())
        ).unwrap();
        let battle = Battle::new(
            replay.config, replay.p1_seed, replay.p2_seed, replay.garbage_seed
        );
        ReplayGame {
            gui: Gui::new(&battle),
            battle,
            updates: replay.updates,
            start_delay: 500,
            resources
        }
    }
}

impl EventHandler for ReplayGame<'_> {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        while timer::check_update_time(ctx, 60) {
            if self.start_delay == 0 {
                if let Some(
                    (p1_controller, p1_info_update, p2_controller, p2_info_update)
                ) = self.updates.pop_front() {
                    let mut update = self.battle.update(p1_controller, p2_controller);
                    update.player_1.info = p1_info_update;
                    update.player_2.info = p2_info_update;
                    self.gui.update(update, self.resources)?;
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
            graphics::queue_text(ctx, &txt, [center-14.0*scale, 9.0*scale], None);
            graphics::queue_text(ctx, &txt, [center+4.0*scale, 9.0*scale], None);
        }

        self.gui.draw(ctx, self.resources, scale, center)?;
        graphics::present(ctx)
    }
}