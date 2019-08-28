use ggez::event::{ self, EventHandler };
use ggez::{ Context, GameResult };
use ggez::graphics::{ self, Image, DrawParam, Rect, FilterMode };
use ggez::timer;
use ggez::graphics::spritebatch::SpriteBatch;
use ggez::input::keyboard::{ KeyCode, is_key_pressed };
use ggez::input::gamepad::{ gamepad, GamepadId };
use ggez::event::{ Button, Axis };
use crate::common::{ GraphicsUpdate, BoardDrawState };

pub struct LocalGame {
    battle: libtetris::Battle,
    player_1_graphics: BoardDrawState,
    player_2_graphics: BoardDrawState,
    image: Image,
    gamepad_p2: Option<GamepadId>
}

impl LocalGame {
    pub fn new(ctx: &mut Context) -> Self {
        let image = Image::new(ctx, "/sprites.png").unwrap();
        let battle = libtetris::Battle::new(Default::default());
        LocalGame {
            player_1_graphics: BoardDrawState::new(battle.player_1.board.next_queue().collect()),
            player_2_graphics: BoardDrawState::new(battle.player_2.board.next_queue().collect()),
            battle,
            image,
            gamepad_p2: None
        }
    }
}

impl EventHandler for LocalGame {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        while timer::check_update_time(ctx, 60) {
            let p1_controller = libtetris::Controller {
                left: is_key_pressed(ctx, KeyCode::Left),
                right: is_key_pressed(ctx, KeyCode::Right),
                rotate_left: is_key_pressed(ctx, KeyCode::Z),
                rotate_right: is_key_pressed(ctx, KeyCode::X),
                soft_drop: is_key_pressed(ctx, KeyCode::Down),
                hard_drop: is_key_pressed(ctx, KeyCode::Space),
                hold: is_key_pressed(ctx, KeyCode::C),
            };

            let p2_controller = match self.gamepad_p2 {
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

            let (events_p1, events_p2) = self.battle.update(p1_controller, p2_controller);

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

        graphics::apply_transformations(ctx)?;
        graphics::draw_queued_text(
            ctx, DrawParam::new(), None, FilterMode::Linear
        )?;
        graphics::present(ctx)
    }

    fn gamepad_button_down_event(&mut self, _: &mut Context, _: Button, id: GamepadId) {
        if self.gamepad_p2.is_none() {
            self.gamepad_p2 = Some(id);
        }
    }
}
