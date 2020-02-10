use libtetris::{ PieceMovement, Board, Row, FallingPiece, Controller };
use std::collections::VecDeque;
use crate::Event;

pub struct PieceMoveExecutor {
    needs_hold: bool,
    executing: VecDeque<PieceMovement>
}

impl PieceMoveExecutor {
    pub fn new(hold: bool, to_do: VecDeque<PieceMovement>) -> Self {
        PieceMoveExecutor {
            needs_hold: hold,
            executing: to_do
        }
    }

    pub fn update<R: Row>(
        &mut self, controller: &mut Controller, board: &Board<R>, events: &[Event]
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
                                controller.hard_drop = true;
                            }
                            Some(PieceMovement::SonicDrop) => {
                                controller.right = false;
                                controller.rotate_left = false;
                                controller.rotate_right = false;
                                controller.left = false;

                                controller.soft_drop = true;
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