use std::collections::HashSet;

use battle::{Event, PieceMoveExecutor};
use game_util::winit::event::VirtualKeyCode;
use gilrs::{Axis, Button, Gamepad};
use libtetris::*;
use serde::{Deserialize, Serialize};

pub trait InputSource {
    fn controller(&self, keys: &HashSet<VirtualKeyCode>, gamepad: Option<Gamepad>) -> Controller;
    fn update(
        &mut self,
        board: &Board<ColoredRow>,
        events: &[Event],
        incoming: u32,
    ) -> Option<cold_clear::Info>;
}

pub struct BotInput {
    interface: cold_clear::Interface,
    executing: Option<(FallingPiece, PieceMoveExecutor)>,
    controller: Controller,
    speed_limit: u32,
}

impl BotInput {
    pub fn new(interface: cold_clear::Interface, speed_limit: u32) -> Self {
        BotInput {
            interface,
            executing: None,
            controller: Default::default(),
            speed_limit,
        }
    }
}

impl InputSource for BotInput {
    fn controller(&self, _keys: &HashSet<VirtualKeyCode>, _gamepad: Option<Gamepad>) -> Controller {
        self.controller
    }

    fn update(
        &mut self,
        board: &Board<ColoredRow>,
        events: &[Event],
        incoming: u32,
    ) -> Option<cold_clear::Info> {
        for event in events {
            match event {
                Event::PieceSpawned { new_in_queue } => {
                    self.interface.add_next_piece(*new_in_queue);
                    if self.executing.is_none() {
                        self.interface.suggest_next_move(incoming);
                    }
                }
                Event::GarbageAdded(_) => {
                    self.interface
                        .reset(board.get_field(), board.b2b_bonus, board.combo);
                }
                _ => {}
            }
        }
        let mut info = None;
        if let Ok((mv, i)) = self.interface.poll_next_move() {
            self.interface.play_next_move(mv.expected_location);
            info = Some(i);
            self.executing = Some((
                mv.expected_location,
                PieceMoveExecutor::new(mv.hold, mv.inputs.into_iter().collect(), self.speed_limit),
            ));
        }
        if let Some((expected, ref mut executor)) = self.executing {
            if let Some(loc) = executor.update(&mut self.controller, board, events) {
                if loc != expected {
                    self.interface
                        .reset(board.get_field(), board.b2b_bonus, board.combo);
                }
                self.executing = None;
            }
        }
        info
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Default, Debug)]
pub struct UserInput {
    keyboard: Config<VirtualKeyCode>,
    gamepad: Config<GamepadControl>,
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
struct Config<T> {
    left: T,
    right: T,
    rotate_left: T,
    rotate_right: T,
    hard_drop: T,
    soft_drop: T,
    hold: T,
}

impl Default for Config<VirtualKeyCode> {
    fn default() -> Self {
        Config {
            left: VirtualKeyCode::Left,
            right: VirtualKeyCode::Right,
            rotate_left: VirtualKeyCode::Z,
            rotate_right: VirtualKeyCode::X,
            hard_drop: VirtualKeyCode::Space,
            soft_drop: VirtualKeyCode::Down,
            hold: VirtualKeyCode::C,
        }
    }
}

impl Default for Config<GamepadControl> {
    fn default() -> Self {
        Config {
            left: GamepadControl::Button(Button::DPadLeft),
            right: GamepadControl::Button(Button::DPadRight),
            rotate_left: GamepadControl::Button(Button::South),
            rotate_right: GamepadControl::Button(Button::East),
            hard_drop: GamepadControl::Button(Button::DPadUp),
            soft_drop: GamepadControl::Button(Button::DPadDown),
            hold: GamepadControl::Button(Button::LeftTrigger),
        }
    }
}

impl InputSource for UserInput {
    fn controller(&self, keys: &HashSet<VirtualKeyCode>, gamepad: Option<Gamepad>) -> Controller {
        Controller {
            left: self.read_input(keys, gamepad, self.keyboard.left, self.gamepad.left),
            right: self.read_input(keys, gamepad, self.keyboard.right, self.gamepad.right),
            rotate_left: self.read_input(
                keys,
                gamepad,
                self.keyboard.rotate_left,
                self.gamepad.rotate_left,
            ),
            rotate_right: self.read_input(
                keys,
                gamepad,
                self.keyboard.rotate_right,
                self.gamepad.rotate_right,
            ),
            hard_drop: self.read_input(
                keys,
                gamepad,
                self.keyboard.hard_drop,
                self.gamepad.hard_drop,
            ),
            soft_drop: self.read_input(
                keys,
                gamepad,
                self.keyboard.soft_drop,
                self.gamepad.soft_drop,
            ),
            hold: self.read_input(keys, gamepad, self.keyboard.hold, self.gamepad.hold),
        }
    }

    fn update(&mut self, _: &Board<ColoredRow>, _: &[Event], _: u32) -> Option<cold_clear::Info> {
        None
    }
}

impl UserInput {
    fn read_input(
        &self,
        keys: &HashSet<VirtualKeyCode>,
        controller: Option<Gamepad>,
        keyboard: VirtualKeyCode,
        gamepad: GamepadControl,
    ) -> bool {
        keys.contains(&keyboard)
            || controller.map_or(false, |c| match gamepad {
                GamepadControl::Button(button) => c.is_pressed(button),
                GamepadControl::PositiveAxis(axis) => c.value(axis) > 0.5,
                GamepadControl::NegativeAxis(axis) => c.value(axis) < -0.5,
            })
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
enum GamepadControl {
    Button(Button),
    NegativeAxis(Axis),
    PositiveAxis(Axis),
}
