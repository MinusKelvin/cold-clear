use ggez::Context;
use ggez::input::keyboard::{ KeyCode, is_key_pressed };
use libtetris::*;
use battle::{ Controller, Event, PieceMoveExecutor };

pub trait InputSource {
    fn controller(&mut self, ctx: &mut Context) -> Controller;
    fn update(&mut self, board: &Board<ColoredRow>, events: &[Event]) -> Option<bot::Info>;
}

pub struct BotInput {
    interface: bot::Interface,
    executing: Option<(FallingPiece, PieceMoveExecutor)>,
    controller: Controller
}

impl BotInput {
    pub fn new(interface: bot::Interface) -> Self {
        BotInput {
            interface,
            executing: None,
            controller: Default::default()
        }
    }
}

impl InputSource for BotInput {
    fn controller(&mut self, _: &mut Context) -> Controller {
        self.controller
    }

    fn update(&mut self, board: &Board<ColoredRow>, events: &[Event]) -> Option<bot::Info> {
        for event in events {
            match event {
                Event::PieceSpawned { new_in_queue } => {
                    self.interface.add_next_piece(*new_in_queue);
                }
                Event::FrameBeforePieceSpawns => {
                    if self.executing.is_none() {
                        self.interface.request_next_move();
                    }
                }
                Event::GarbageAdded(_) => {
                    self.interface.reset(board.get_field(), board.b2b_bonus, board.combo);
                }
                _ => {}
            }
        }
        let mut info = None;
        if let Some((expected, ref mut executor)) = self.executing {
            if let Some(loc) = executor.update(&mut self.controller, board, events) {
                if loc != expected {
                    self.interface.reset(board.get_field(), board.b2b_bonus, board.combo);
                }
                self.executing = None;
            }
        } else if let Some((mv, i)) = self.interface.poll_next_move() {
            info = Some(i);
            self.executing = Some((
                mv.expected_location,
                PieceMoveExecutor::new(mv.hold, mv.inputs.into_iter().collect())
            ));
        }
        info
    }
}

#[derive(Default, Copy, Clone)]
pub struct Keyboard;

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

    fn update(&mut self, _: &Board<ColoredRow>, _: &[Event]) -> Option<bot::Info> {
        None
    }
}