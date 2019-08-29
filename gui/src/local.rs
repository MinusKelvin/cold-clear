use ggez::event::{ self, EventHandler };
use ggez::{ Context, GameResult };
use ggez::graphics::{ self, Image, DrawParam, Rect, FilterMode, Color };
use ggez::timer;
use ggez::graphics::spritebatch::SpriteBatch;
use ggez::input::keyboard::{ KeyCode, is_key_pressed };
use ggez::input::gamepad::{ gamepad, GamepadId };
use ggez::event::{ Button, Axis };
use crate::common::{ GraphicsUpdate, BoardDrawState, text };

pub struct LocalGame {
    battle: libtetris::Battle,
    player_1_graphics: BoardDrawState,
    player_2_graphics: BoardDrawState,
    p1_bot: bot::BotController,
    p2_bot: bot::BotController,
    p1_wins: u32,
    p2_wins: u32,
    image: Image,
    state: State
}

enum State {
    Playing,
    GameOver(u32),
    Starting(u32)
}

impl LocalGame {
    pub fn new(ctx: &mut Context) -> Self {
        let image = Image::new(ctx, "/sprites.png").unwrap();
        let battle = libtetris::Battle::new(Default::default());
        LocalGame {
            player_1_graphics: BoardDrawState::new(battle.player_1.board.next_queue().collect()),
            player_2_graphics: BoardDrawState::new(battle.player_2.board.next_queue().collect()),
            p1_bot: bot::BotController::new(battle.player_1.board.next_queue()),
            p2_bot: bot::BotController::new(battle.player_2.board.next_queue()),
            p1_wins: 0,
            p2_wins: 0,
            battle,
            image,
            state: State::Starting(180)
        }
    }
}

impl EventHandler for LocalGame {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        while timer::check_update_time(ctx, 60) {
            let do_update = match self.state {
                State::GameOver(0) => {
                    self.battle = libtetris::Battle::new(libtetris::GameConfig {
                        margin_time: Some(60),
                        ..Default::default()
                    });
                    self.player_1_graphics = BoardDrawState::new(
                        self.battle.player_1.board.next_queue().collect()
                    );
                    self.p1_bot = bot::BotController::new(self.battle.player_1.board.next_queue());
                    self.player_2_graphics = BoardDrawState::new(
                        self.battle.player_2.board.next_queue().collect()
                    );
                    self.p2_bot = bot::BotController::new(self.battle.player_2.board.next_queue());
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

                let (events_p1, events_p2) = self.battle.update(p1_controller, p2_controller);

                self.p1_bot.update(&events_p1, &self.battle.player_1.board);
                self.p2_bot.update(&events_p2, &self.battle.player_2.board);

                if let State::Playing = self.state {
                    for event in &events_p1 {
                        use libtetris::Event::*;
                        match event {
                            GameOver => {
                                self.p2_wins += 1;
                                self.state = State::GameOver(300);
                            }
                            _ => {}
                        }
                    }
                    for event in &events_p2 {
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

                self.player_1_graphics.update(GraphicsUpdate {
                    events: events_p1,
                    garbage_queue: self.battle.player_1.garbage_queue,
                    statistics: self.battle.player_1.board.statistics,
                    game_time: self.battle.time
                });
                self.player_2_graphics.update(GraphicsUpdate {
                    events: events_p2,
                    garbage_queue: self.battle.player_2.garbage_queue,
                    statistics: self.battle.player_2.board.statistics,
                    game_time: self.battle.time
                });
            }
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        graphics::clear(ctx, graphics::BLACK);
        let size = graphics::drawable_size(ctx);
        let center = size.0 / 2.0;
        let scale = size.1 / 25.0;
        graphics::set_screen_coordinates(ctx, Rect {
            x: 0.0, y: 0.0, w: size.0, h: size.1
        })?;

        graphics::push_transform(ctx, Some(DrawParam::new()
            .scale([scale, scale])
            .dest([center - 16.0 * scale, 0.0])
            .to_matrix()));
        graphics::apply_transformations(ctx)?;
        self.player_1_graphics.draw(ctx, &self.image, center - 16.0*scale, scale)?;
        graphics::pop_transform(ctx);

        graphics::push_transform(ctx, Some(DrawParam::new()
            .scale([scale, scale])
            .dest([center, 0.0])
            .to_matrix()));
        graphics::apply_transformations(ctx)?;
        self.player_2_graphics.draw(ctx, &self.image, center, scale)?;
        graphics::pop_transform(ctx);

        graphics::queue_text(
            ctx,
            &text(format!("{} - {}", self.p1_wins, self.p2_wins), scale*1.5, 6.0*scale),
            [center-3.0*scale, 20.5*scale],
            None
        );
        graphics::queue_text(
            ctx,
            &text(
                format!("{}:{:02}", self.battle.time / 60 / 60, self.battle.time / 60 % 60),
                scale*1.0, 6.0*scale
            ),
            [center-3.0*scale, 22.0*scale],
            None
        );
        if self.battle.multiplier != 1.0 {
            graphics::queue_text(
                ctx,
                &text("Margin Time", scale*1.0, 6.0*scale),
                [center-3.0*scale, 23.0*scale],
                None
            );
            graphics::queue_text(
                ctx,
                &text(format!("Attack x{:.1}", self.battle.multiplier), scale*1.0, 6.0*scale),
                [center-3.0*scale, 23.8*scale],
                Some(Color::from_rgb(255, 64, 32))
            );
        }

        graphics::apply_transformations(ctx)?;
        graphics::draw_queued_text(
            ctx, DrawParam::new(), None, FilterMode::Linear
        )?;
        graphics::present(ctx)
    }

    // fn gamepad_button_down_event(&mut self, _: &mut Context, _: Button, id: GamepadId) {
    //     if self.gamepad_p2.is_none() {
    //         self.gamepad_p2 = Some(id);
    //     }
    // }
}
