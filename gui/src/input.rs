use ggez::Context;
use ggez::input::keyboard::{ KeyCode, is_key_pressed };
use ggez::input::gamepad::Gamepad;
use ggez::event::{ Button, Axis };
use libtetris::*;
use battle::{ Event, PieceMoveExecutor };
use serde::{ Serialize, Deserialize };

pub trait InputSource {
    fn controller(&mut self, ctx: &Context, gamepad: Option<Gamepad>) -> Controller;
    fn update(
        &mut self, board: &Board<ColoredRow>, events: &[Event], incoming: u32
    ) -> Option<cold_clear::Info>;
}

pub struct BotInput {
    interface: cold_clear::Interface,
    executing: Option<(FallingPiece, PieceMoveExecutor)>,
    controller: Controller
}

impl BotInput {
    pub fn new(interface: cold_clear::Interface) -> Self {
        BotInput {
            interface,
            executing: None,
            controller: Default::default()
        }
    }
}

impl InputSource for BotInput {
    fn controller(&mut self, _: &Context, _: Option<Gamepad>) -> Controller {
        self.controller
    }

    fn update(
        &mut self, board: &Board<ColoredRow>, events: &[Event], incoming: u32
    ) -> Option<cold_clear::Info> {
        for event in events {
            match event {
                Event::PieceSpawned { new_in_queue } => {
                    self.interface.add_next_piece(*new_in_queue);
                }
                Event::FrameBeforePieceSpawns => {
                    if self.executing.is_none() {
                        self.interface.request_next_move(incoming);
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
            hold: GamepadControl::Button(Button::LeftTrigger)
        }
    }
}

impl InputSource for UserInput {
    fn controller(&mut self, ctx: &Context, c: Option<Gamepad>) -> Controller {
        Controller {
            left: read_input(ctx, c, self.keyboard.left, self.gamepad.left),
            right: read_input(ctx, c, self.keyboard.right, self.gamepad.right),
            rotate_left: read_input(ctx, c, self.keyboard.rotate_left, self.gamepad.rotate_left),
            rotate_right: read_input(ctx, c, self.keyboard.rotate_right, self.gamepad.rotate_right),
            hard_drop: read_input(ctx, c, self.keyboard.hard_drop, self.gamepad.hard_drop),
            soft_drop: read_input(ctx, c, self.keyboard.soft_drop, self.gamepad.soft_drop),
            hold: read_input(ctx, c, self.keyboard.hold, self.gamepad.hold),
        }
    }

    fn update(&mut self, _: &Board<ColoredRow>, _: &[Event], _: u32) -> Option<cold_clear::Info> {
        None
    }
}

fn read_input(
    ctx: &Context, controller: Option<Gamepad>, keyboard: KeyCode, gamepad: GamepadControl
) -> bool {
    is_key_pressed(ctx, keyboard) || controller.map_or(false, |c| match gamepad {
        GamepadControl::Button(button) => c.is_pressed(button),
        GamepadControl::PositiveAxis(axis) => c.value(axis) > 0.5,
        GamepadControl::NegativeAxis(axis) => c.value(axis) < -0.5,
    })
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