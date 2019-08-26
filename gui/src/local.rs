use ggez::event::{ self, EventHandler };
use ggez::{ Context, GameResult };
use ggez::graphics::{ self, Image, DrawParam };
use ggez::timer;
use ggez::graphics::spritebatch::SpriteBatch;
use ggez::input::keyboard::{ KeyCode, is_key_pressed };
use crate::common::BoardDrawState;

pub struct LocalGame {
    player_1: Player,
    player_1_graphics: BoardDrawState,
    player_2: Player,
    image: Image,
}

impl LocalGame {
    pub fn new(ctx: &mut Context) -> Self {
        let image = Image::new(ctx, "/sprites.png").unwrap();
        LocalGame {
            player_1: Player::new(),
            player_1_graphics: BoardDrawState::new(),
            player_2: Player::new(),
            image,
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
            let events = self.player_1.game.update(controller);
            self.player_1_graphics.update(&events);
            for event in events {
                use libtetris::Event::*;
                match event {
                    GameOver => ggez::event::quit(ctx),
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let scale = graphics::size(ctx).1 / 25.0;
        graphics::push_transform(ctx, Some(DrawParam::new()
            .scale([scale, scale])
            .to_matrix()));
        graphics::apply_transformations(ctx);
        self.player_1_graphics.draw(ctx, &self.image)?;
        graphics::pop_transform(ctx);
        graphics::present(ctx)
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