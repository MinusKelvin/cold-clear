use std::collections::VecDeque;

use libtetris::{Board, Controller, FallingPiece, PieceMovement, Row};

use crate::Event;

pub struct PieceMoveExecutor {
    needs_hold: bool,
    speed_limit: u32,
    input_timer: u32,
    executing: VecDeque<PieceMovement>,
    controller: Controller,
}

impl PieceMoveExecutor {
    pub fn new(hold: bool, to_do: VecDeque<PieceMovement>, speed_limit: u32) -> Self {
        PieceMoveExecutor {
            needs_hold: hold,
            executing: to_do,
            speed_limit,
            input_timer: speed_limit,
            controller: Default::default(),
        }
    }

    pub fn update<R: Row>(
        &mut self,
        controller: &mut Controller,
        board: &Board<R>,
        events: &[Event],
    ) -> Option<FallingPiece> {
        for event in events {
            match event {
                Event::PieceHeld(_) => {
                    self.needs_hold = false;
                }
                Event::PieceFalling(piece, _) => {
                    if self.input_timer == 0 || self.controller.soft_drop {
                        if self.needs_hold {
                            self.controller.hold ^= true;
                        } else {
                            self.controller.hold = false;
                            self.controller.hard_drop = false;
                            match self.executing.front() {
                                None => {
                                    self.controller = Default::default();
                                    self.controller.hard_drop = true;
                                }
                                Some(PieceMovement::SonicDrop) => {
                                    self.controller.right = false;
                                    self.controller.rotate_left = false;
                                    self.controller.rotate_right = false;
                                    self.controller.left = false;

                                    self.controller.soft_drop = true;
                                    if board.on_stack(piece) {
                                        self.executing.pop_front();
                                        self.controller.soft_drop = false;
                                    }
                                }
                                Some(PieceMovement::Left) => {
                                    self.controller.right = false;
                                    self.controller.rotate_left = false;
                                    self.controller.rotate_right = false;
                                    self.controller.soft_drop = false;

                                    self.controller.left ^= true;
                                    if self.controller.left {
                                        self.executing.pop_front();
                                    }
                                }
                                Some(PieceMovement::Right) => {
                                    self.controller.left = false;
                                    self.controller.rotate_left = false;
                                    self.controller.rotate_right = false;
                                    self.controller.soft_drop = false;

                                    self.controller.right ^= true;
                                    if self.controller.right {
                                        self.executing.pop_front();
                                    }
                                }
                                Some(PieceMovement::Cw) => {
                                    self.controller.right = false;
                                    self.controller.rotate_left = false;
                                    self.controller.left = false;
                                    self.controller.soft_drop = false;

                                    self.controller.rotate_right ^= true;
                                    if self.controller.rotate_right {
                                        self.executing.pop_front();
                                    }
                                }
                                Some(PieceMovement::Ccw) => {
                                    self.controller.left = false;
                                    self.controller.right = false;
                                    self.controller.rotate_right = false;
                                    self.controller.soft_drop = false;

                                    self.controller.rotate_left ^= true;
                                    if self.controller.rotate_left {
                                        self.executing.pop_front();
                                    }
                                }
                            }
                        }
                        self.input_timer = self.speed_limit;
                        *controller = self.controller;
                    } else {
                        self.input_timer -= 1;
                        *controller = Default::default();
                    }
                }
                Event::PiecePlaced { piece, .. } => {
                    self.controller.hard_drop = false;
                    *controller = Default::default();
                    return Some(*piece);
                }
                _ => {}
            }
        }
        None
    }
}
