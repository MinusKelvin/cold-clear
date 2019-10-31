use serde::{ Serialize, Deserialize };
use libtetris::{ PieceMovement, Board, Row, FallingPiece };
use std::collections::VecDeque;
use crate::Event;

#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
pub struct Controller {
    pub left: bool,
    pub right: bool,
    pub rotate_right: bool,
    pub rotate_left: bool,
    pub soft_drop: bool,
    pub hard_drop: bool,
    pub hold: bool
}

impl Serialize for Controller {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(
            (self.left as u8)         << 1 |
            (self.right as u8)        << 2 |
            (self.rotate_left as u8)  << 3 |
            (self.rotate_right as u8) << 4 |
            (self.hold as u8)         << 5 |
            (self.soft_drop as u8)    << 6 |
            (self.hard_drop as u8)    << 7
        )
    }
}

impl<'de> Deserialize<'de> for Controller {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ControllerDeserializer;
        impl serde::de::Visitor<'_> for ControllerDeserializer {
            type Value = Controller;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a byte-sized bit vector")
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Controller, E> {
                Ok(Controller {
                    left:         (v >> 1) & 1 != 0,
                    right:        (v >> 2) & 1 != 0,
                    rotate_left:  (v >> 3) & 1 != 0,
                    rotate_right: (v >> 4) & 1 != 0,
                    hold:         (v >> 5) & 1 != 0,
                    soft_drop:    (v >> 6) & 1 != 0,
                    hard_drop:    (v >> 7) & 1 != 0,
                })
            }
        }
        deserializer.deserialize_u8(ControllerDeserializer)
    }
}

pub struct PieceMoveExecutor {
    needs_hold: bool,
    stall_for: u32,
    executing: VecDeque<PieceMovement>
}

impl PieceMoveExecutor {
    pub fn new(hold: bool, stall_for: u32, to_do: VecDeque<PieceMovement>) -> Self {
        PieceMoveExecutor {
            needs_hold: hold,
            stall_for,
            executing: to_do
        }
    }

    pub fn update<R: Row>(
        &mut self,
        controller: &mut Controller,
        board: &Board<R>,
        events: &[Event],
        garbage_queue: u32
    ) -> Option<FallingPiece> {
        for event in events {
            match event {
                Event::PieceHeld(_) => {
                    self.needs_hold = false;
                }
                Event::PieceFalling(piece, _) => {
                    if self.needs_hold {
                        controller.hold ^= true;
                    } else {
                        controller.hold = false;
                        controller.hard_drop = false;
                        match self.executing.front() {
                            None => {
                                *controller = Default::default();
                                controller.hard_drop = self.stall_for <= garbage_queue;
                            }
                            Some(PieceMovement::SonicDrop) => {
                                controller.right = false;
                                controller.rotate_left = false;
                                controller.rotate_right = false;
                                controller.left = false;

                                controller.soft_drop = self.stall_for <= garbage_queue;
                                if board.on_stack(piece) {
                                    self.executing.pop_front();
                                }
                            }
                            Some(PieceMovement::Left) => {
                                controller.right = false;
                                controller.rotate_left = false;
                                controller.rotate_right = false;
                                controller.soft_drop = false;
                                
                                controller.left ^= true;
                                if controller.left {
                                    self.executing.pop_front();
                                }
                            }
                            Some(PieceMovement::Right) => {
                                controller.left = false;
                                controller.rotate_left = false;
                                controller.rotate_right = false;
                                controller.soft_drop = false;
                                
                                controller.right ^= true;
                                if controller.right {
                                    self.executing.pop_front();
                                }
                            }
                            Some(PieceMovement::Cw) => {
                                controller.right = false;
                                controller.rotate_left = false;
                                controller.left = false;
                                controller.soft_drop = false;
                                
                                controller.rotate_right ^= true;
                                if controller.rotate_right {
                                    self.executing.pop_front();
                                }
                            }
                            Some(PieceMovement::Ccw) => {
                                controller.left = false;
                                controller.right = false;
                                controller.rotate_right = false;
                                controller.soft_drop = false;
                                
                                controller.rotate_left ^= true;
                                if controller.rotate_left {
                                    self.executing.pop_front();
                                }
                            }
                        }
                    }
                }
                Event::PiecePlaced { piece, .. } => {
                    controller.hard_drop = false;
                    return Some(*piece)
                }
                _ => {}
            }
        }
        None
    }
}