use ggez::Context;
use ggez::input::keyboard::{ KeyCode, is_key_pressed };
use libtetris::*;

pub trait InputSource {
    fn controller(&mut self, ctx: &mut Context) -> Controller;
    fn update(&mut self, board: &Board<ColoredRow>, events: &[Event]) -> Option<Info>;
}

impl InputSource for bot::Controller {
    fn controller(&mut self, _: &mut Context) -> Controller {
        self.controller()
    }

    fn update(&mut self, board: &Board<ColoredRow>, events: &[Event]) -> Option<Info> {
        self.update(board, events)
    }
}

#[derive(Default, Copy, Clone)]
pub struct Keyboard(bool);

impl InputSource for Keyboard {
    fn controller(&mut self, ctx: &mut Context) -> Controller {
        Controller {
            left: is_key_pressed(ctx, KeyCode::Left),
            right: is_key_pressed(ctx, KeyCode::Right),
            rotate_left: is_key_pressed(ctx, KeyCode::Z),
            rotate_right: is_key_pressed(ctx, KeyCode::X),
            hard_drop: is_key_pressed(ctx, KeyCode::Space),
            soft_drop: is_key_pressed(ctx, KeyCode::Down),
            hold: is_key_pressed(ctx, KeyCode::C),
        }
    }

    fn update(&mut self, _: &Board<ColoredRow>, _: &[Event]) -> Option<Info> {
        if self.0 {
            None
        } else {
            self.0 = true;
            Some(vec![("Human".to_owned(), None)])
        }
    }
}