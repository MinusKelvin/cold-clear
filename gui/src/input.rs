use ggez::Context;
use ggez::input::keyboard::{ KeyCode, is_key_pressed };
use ggez::input::gamepad::Gamepad;
use ggez::event::{ Button, Axis };
use libtetris::*;
use battle::{ Controller, Event, PieceMoveExecutor };
use serde::{ Serialize, Deserialize };

pub trait InputSource {
    fn controller(&mut self, ctx: &mut Context) -> Controller;
    fn update(
        &mut self, board: &Board<ColoredRow>, events: &[Event], gamepad: Option<Gamepad>,
    ) -> Option<bot::Info>;
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

    fn update(
        &mut self, board: &Board<ColoredRow>, events: &[Event], _: Option<Gamepad>
    ) -> Option<bot::Info> {
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

#[derive(Copy, Clone, Serialize, Deserialize, Default, Debug)]
pub struct UserInput {
    keyboard: KeyboardConfig,
    gamepad: GamepadConfig,
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
struct KeyboardConfig {
    left: KeyCode,
    right: KeyCode,
    rotate_left: KeyCode,
    rotate_right: KeyCode,
    hard_drop: KeyCode,
    soft_drop: KeyCode,
    hold: KeyCode
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
struct GamepadConfig {
    left: GamepadControl,
    right: GamepadControl,
    rotate_left: GamepadControl,
    rotate_right: GamepadControl,
    hard_drop: GamepadControl,
    soft_drop: GamepadControl,
    hold: GamepadControl
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        KeyboardConfig {
            left: KeyCode::Left,
            right: KeyCode::Right,
            rotate_left: KeyCode::Z,
            rotate_right: KeyCode::X,
            hard_drop: KeyCode::Space,
            soft_drop: KeyCode::Down,
            hold: KeyCode::C,
        }
    }
}

impl Default for GamepadConfig {
    fn default() -> Self {
        GamepadConfig {
            left: GamepadControl::Button(Button::DPadLeft),
            right: GamepadControl::Button(Button::DPadRight),
            rotate_left: GamepadControl::Button(Button::South),
            rotate_right: GamepadControl::Button(Button::East),
            hard_drop: GamepadControl::Button(Button::DPadUp),
            soft_drop: GamepadControl::Button(Button::DPadDown),
            hold: GamepadControl::PositiveAxis(Axis::LeftStickX)
        }
    }
}

impl InputSource for UserInput {
    fn controller(&mut self, ctx: &mut Context) -> Controller {
        Controller {
            left: is_key_pressed(ctx, self.keyboard.left),
            right: is_key_pressed(ctx, self.keyboard.right),
            rotate_left: is_key_pressed(ctx, self.keyboard.rotate_left),
            rotate_right: is_key_pressed(ctx, self.keyboard.rotate_right),
            hard_drop: is_key_pressed(ctx, self.keyboard.hard_drop),
            soft_drop: is_key_pressed(ctx, self.keyboard.soft_drop),
            hold: is_key_pressed(ctx, self.keyboard.hold),
        }
    }

    fn update(
        &mut self, _: &Board<ColoredRow>, _: &[Event], gamepad: Option<Gamepad>
    ) -> Option<bot::Info> {
        None
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
enum GamepadControl {
    #[serde(with = "ButtonDef")]
    Button(Button),
    #[serde(with = "AxisDef")]
    NegativeAxis(Axis),
    #[serde(with = "AxisDef")]
    PositiveAxis(Axis)
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Button")]
enum ButtonDef {
    South,
    East,
    North,
    West,
    C,
    Z,
    LeftTrigger,
    LeftTrigger2,
    RightTrigger,
    RightTrigger2,
    Select,
    Start,
    Mode,
    LeftThumb,
    RightThumb,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Unknown
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Axis")]
enum AxisDef {
    LeftStickX,
    LeftStickY,
    LeftZ,
    RightStickX,
    RightStickY,
    RightZ,
    DPadX,
    DPadY,
    Unknown
}