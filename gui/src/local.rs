use ggez::event::{ self, EventHandler };
use ggez::{ Context, GameResult };
use ggez::graphics::{ self, Image, DrawParam, Rect, FilterMode, Color };
use ggez::audio::{ self, SoundSource };
use ggez::timer;
use ggez::graphics::spritebatch::SpriteBatch;
use ggez::input::keyboard::{ KeyCode, is_key_pressed };
use ggez::input::gamepad::{ gamepad, GamepadId };
use ggez::event::{ Button, Axis };
use crate::interface::{ Gui, text };
use serde::{ Serialize, Deserialize };
use std::collections::VecDeque;

pub struct LocalGame {
    battle: libtetris::Battle,
    p1_bot: bot::BotController,
    p2_bot: bot::BotController,
    p1_wins: u32,
    p2_wins: u32,
    gui: Gui,
    state: State,
}

enum State {
    Playing,
    GameOver(u32),
    Starting(u32)
}

impl LocalGame {
    pub fn new(ctx: &mut Context) -> Self {
        let battle = libtetris::Battle::new(Default::default());
        let p1_pieces: VecDeque<_> = battle.player_1.board.next_queue().collect();
        let p2_pieces: VecDeque<_> = battle.player_2.board.next_queue().collect();
        LocalGame {
            p1_bot: bot::BotController::new(p1_pieces.iter().copied(), false),
            p2_bot: bot::BotController::new(p2_pieces.iter().copied(), true),
            p1_wins: 0,
            p2_wins: 0,
            gui: Gui::new(ctx, p1_pieces, p2_pieces),
            battle,
            state: State::Starting(500),
        }
    }
}

impl EventHandler for LocalGame {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        while timer::check_update_time(ctx, 60) {
            let do_update = match self.state {
                State::GameOver(0) => {
                    let t = std::time::Instant::now();
                    serde_json::to_writer(
                        std::io::BufWriter::new(
                            std::fs::File::create("test-replay.json"
                        ).unwrap()),
                        &self.battle.replay
                    ).unwrap();
                    println!("Took {:?} to save replay", t.elapsed());

                    // Don't catch up after pause due to replay saving
                    while timer::check_update_time(ctx, 60) {}

                    self.battle = libtetris::Battle::new(Default::default());

                    let p1_pieces: VecDeque<_> = self.battle.player_1.board.next_queue().collect();
                    let p2_pieces: VecDeque<_> = self.battle.player_2.board.next_queue().collect();

                    self.p1_bot = bot::BotController::new(p1_pieces.clone(), false);
                    self.p2_bot = bot::BotController::new(p2_pieces.clone(), true);

                    self.gui.reset(p1_pieces, p2_pieces);

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
                let p1_controller = self.p1_bot.controller();
                let p2_controller = self.p2_bot.controller();

                let mut update = self.battle.update(p1_controller, p2_controller);

                update.player_1.info = self.p1_bot.update(
                    &update.player_1.events, &self.battle.player_1.board
                );
                update.player_2.info = self.p2_bot.update(
                    &update.player_2.events, &self.battle.player_2.board
                );

                if let State::Playing = self.state {
                    for event in &update.player_1.events {
                        use libtetris::Event::*;
                        match event {
                            GameOver => {
                                self.p2_wins += 1;
                                self.state = State::GameOver(300);
                            }
                            _ => {}
                        }
                    }
                    for event in &update.player_2.events {
                        use libtetris::Event::*;
                        match event {
                            GameOver => {
                                self.p1_wins += 1;
                                self.state = State::GameOver(300);
                            }
                            _ => {}
                        }
                    }
                }

                self.gui.update(update)?;
            }
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let (scale, center) = crate::interface::setup_graphics(ctx)?;

        graphics::queue_text(
            ctx,
            &text(format!("{} - {}", self.p1_wins, self.p2_wins), scale*2.0, 6.0*scale),
            [center-3.0*scale, 17.0*scale],
            None
        );

        if let State::Starting(t) = self.state {
            let txt = text(format!("{}", t / 60 + 1), scale * 4.0, 10.0*scale);
            graphics::queue_text(ctx, &txt, [center-14.0*scale, 9.0*scale], None);
            graphics::queue_text(ctx, &txt, [center+4.0*scale, 9.0*scale], None);
        }

        self.gui.draw(ctx, scale, center)?;

        graphics::present(ctx)
    }

    // fn gamepad_button_down_event(&mut self, _: &mut Context, _: Button, id: GamepadId) {
    //     if self.gamepad_p2.is_none() {
    //         self.gamepad_p2 = Some(id);
    //     }
    // }
}
