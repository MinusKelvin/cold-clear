use ggez::event::{ self, EventHandler };
use ggez::{ Context, GameResult };
use ggez::graphics::{ self, Image, DrawParam, Rect };
use ggez::timer;
use ggez::graphics::spritebatch::SpriteBatch;
use ggez::input::keyboard::{ KeyCode, is_key_pressed };
use ggez::input::gamepad::{ gamepad, GamepadId };
use ggez::event::{ Button, Axis };
use crate::common::BoardDrawState;

pub struct LocalGame {
    player_1: Player,
    player_1_graphics: BoardDrawState,
    player_1_attack: Option<u32>,
    player_2: Player,
    player_2_graphics: BoardDrawState,
    player_2_attack: Option<u32>,
    image: Image,
    gamepad_p2: Option<GamepadId>
}

impl LocalGame {
    pub fn new(ctx: &mut Context) -> Self {
        let image = Image::new(ctx, "/sprites.png").unwrap();
        LocalGame {
            player_1: Player::new(),
            player_1_graphics: BoardDrawState::new(),
            player_1_attack: None,
            player_2: Player::new(),
            player_2_graphics: BoardDrawState::new(),
            player_2_attack: None,
            image,
            gamepad_p2: None
        }
    }
}

impl EventHandler for LocalGame {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        while timer::check_update_time(ctx, 60) {
            let controller = libtetris::Controller {
                left: is_key_pressed(ctx, KeyCode::Left),
                right: is_key_pressed(ctx, KeyCode::Right),
                rotate_left: is_key_pressed(ctx, KeyCode::Z),
                rotate_right: is_key_pressed(ctx, KeyCode::X),
                soft_drop: is_key_pressed(ctx, KeyCode::Down),
                hard_drop: is_key_pressed(ctx, KeyCode::Space),
                hold: is_key_pressed(ctx, KeyCode::C),
            };
            let events_p1 = self.player_1.game.update(controller);
            self.player_1_graphics.update(&events_p1);

            let controller = match self.gamepad_p2 {
                None => Default::default(),
                Some(id) => {
                    let gp = gamepad(ctx, id);
                    libtetris::Controller {
                        left: gp.is_pressed(Button::DPadLeft) || gp.value(Axis::LeftStickX) < -0.5,
                        right: gp.is_pressed(Button::DPadRight) || gp.value(Axis::LeftStickX) > 0.5,
                        rotate_left: gp.is_pressed(Button::South),
                        rotate_right: gp.is_pressed(Button::East),
                        soft_drop: gp.is_pressed(Button::DPadDown) ||
                                gp.value(Axis::LeftStickY) < -0.5,
                        hard_drop: gp.is_pressed(Button::DPadUp) ||
                                gp.value(Axis::LeftStickY) > 0.5,
                        hold: gp.is_pressed(Button::LeftTrigger) ||
                                gp.is_pressed(Button::RightTrigger)
                    }
                }
            };

            let events_p2 = self.player_2.game.update(controller);
            self.player_2_graphics.update(&events_p2);
            for event in events_p1 {
                use libtetris::Event::*;
                match event {
                    GameOver => ggez::event::quit(ctx),
                    PiecePlaced { locked, .. } => {
                        self.player_1_attack = Some(locked.garbage_sent);
                    }
                    EndOfLineClearDelay => if let Some(attack) = self.player_1_attack {
                        self.player_2.game.garbage_queue += attack;
                    }
                    _ => {}
                }
            }
            for event in events_p2 {
                use libtetris::Event::*;
                match event {
                    GameOver => ggez::event::quit(ctx),
                    PiecePlaced { locked, .. } => {
                        self.player_2_attack = Some(locked.garbage_sent);
                    }
                    EndOfLineClearDelay => if let Some(attack) = self.player_2_attack {
                        self.player_1.game.garbage_queue += attack;
                    }
                    _ => {}
                }
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
            .dest([center - 15.0 * scale, 0.0])
            .to_matrix()));
        graphics::apply_transformations(ctx)?;
        self.player_1_graphics.draw(ctx, &self.image)?;
        graphics::pop_transform(ctx);
        graphics::push_transform(ctx, Some(DrawParam::new()
            .scale([scale, scale])
            .dest([center, 0.0])
            .to_matrix()));
        graphics::apply_transformations(ctx)?;
        self.player_2_graphics.draw(ctx, &self.image)?;
        graphics::present(ctx)
    }

    fn gamepad_button_down_event(&mut self, _: &mut Context, _: Button, id: GamepadId) {
        if self.gamepad_p2.is_none() {
            self.gamepad_p2 = Some(id);
        }
    }
}

struct Player {
    game: libtetris::Game,
}

impl Player {
    fn new() -> Self {
        Player {
            game: libtetris::Game::new(Default::default())
        }
    }
}