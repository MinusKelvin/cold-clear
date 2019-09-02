use ggez::event::EventHandler;
use ggez::{ Context, GameResult };
use ggez::timer;
use ggez::graphics;
use crate::interface::{ Gui, setup_graphics, text };
use libtetris::{ UpdateResult, Replay };
use std::collections::VecDeque;

pub struct ReplayGame {
    gui: Gui,
    updates: VecDeque<UpdateResult>,
    start_delay: u32
}

impl ReplayGame {
    pub fn new(ctx: &mut Context, file: impl AsRef<std::path::Path>) -> Self {
        let replay: Replay = serde_json::from_reader(
            std::io::BufReader::new(std::fs::File::open(file).unwrap())
        ).unwrap();
        ReplayGame {
            gui: Gui::new(ctx, replay.p1_initial_pieces, replay.p2_initial_pieces),
            updates: replay.updates,
            start_delay: 180
        }
    }
}

impl EventHandler for ReplayGame {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        while timer::check_update_time(ctx, 60) {
            if self.start_delay == 0 {
                if let Some(update) = self.updates.pop_front() {
                    self.gui.update(update)?;
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

        self.gui.draw(ctx, scale, center)?;
        graphics::present(ctx)
    }
}