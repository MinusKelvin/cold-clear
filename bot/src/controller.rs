use std::collections::VecDeque;
use crate::Interface;
use crate::moves::Input;
use libtetris::{ Board, Event, Info, Row, FallingPiece };

pub struct Controller {
    executing: Option<(bool, VecDeque<Input>, FallingPiece)>,
    interface: Interface,
    controller: libtetris::Controller,
}

impl Controller {
    pub fn new(interface: Interface) -> Self {
        Controller {
            executing: None,
            interface,
            controller: libtetris::Controller::default(),
        }
    }

    pub fn controller(&mut self) -> libtetris::Controller {
        self.controller
    }

    pub fn update<R: Row>(
        &mut self, board: &Board<R>, events: &[Event]
    ) -> Option<Info> {
        if self.interface.is_dead() {
            self.controller.hard_drop ^= true;
        }
        let mut update_info = None;
        if let Some(mv) = self.interface.poll_next_move() {
            self.executing = Some((mv.hold, mv.inputs.into_iter().collect(), mv.expected_location));
        }
        let mut reset_bot = false;
        for event in events {
            match event {
                Event::PieceSpawned { new_in_queue } => {
                    self.interface.add_next_piece(*new_in_queue);
                    if self.executing.is_none() {
                        self.interface.request_next_move();
                    }
                }
                Event::SpawnDelayStart => if self.executing.is_none() {
                    self.interface.misa_prepare_next_move();
                }
                Event::PieceHeld(_) => if let Some(ref mut exec) = self.executing {
                    exec.0 = false;
                }
                Event::PieceFalling(piece, _) => if let Some(ref mut exec) = self.executing {
                    if exec.0 {
                        self.controller.hold ^= true;
                    } else {
                        self.controller.hold = false;
                        self.controller.hard_drop = false;
                        match exec.1.front() {
                            None => {
                                self.controller = Default::default();
                                self.controller.hard_drop = true;
                            }
                            Some(Input::DasLeft) => {
                                self.controller.right = false;
                                self.controller.rotate_left = false;
                                self.controller.rotate_right = false;
                                self.controller.soft_drop = false;

                                self.controller.left ^= true;
                                let mut p = piece.clone();
                                if !p.shift(board, -1, 0) {
                                    exec.1.pop_front();
                                }
                            }
                            Some(Input::DasRight) => {
                                self.controller.left = false;
                                self.controller.rotate_left = false;
                                self.controller.rotate_right = false;
                                self.controller.soft_drop = false;

                                self.controller.right ^= true;
                                let mut p = piece.clone();
                                if !p.shift(board, 1, 0) {
                                    exec.1.pop_front();
                                }
                            }
                            Some(Input::SonicDrop) => {
                                self.controller.right = false;
                                self.controller.rotate_left = false;
                                self.controller.rotate_right = false;
                                self.controller.left = false;

                                self.controller.soft_drop = true;
                                let mut p = piece.clone();
                                if !p.shift(board, 0, -1) {
                                    exec.1.pop_front();
                                }
                            }
                            Some(Input::Left) => {
                                self.controller.right = false;
                                self.controller.rotate_left = false;
                                self.controller.rotate_right = false;
                                self.controller.soft_drop = false;
                                
                                self.controller.left ^= true;
                                if self.controller.left {
                                    exec.1.pop_front();
                                }
                            }
                            Some(Input::Right) => {
                                self.controller.left = false;
                                self.controller.rotate_left = false;
                                self.controller.rotate_right = false;
                                self.controller.soft_drop = false;
                                
                                self.controller.right ^= true;
                                if self.controller.right {
                                    exec.1.pop_front();
                                }
                            }
                            Some(Input::Cw) => {
                                self.controller.right = false;
                                self.controller.rotate_left = false;
                                self.controller.left = false;
                                self.controller.soft_drop = false;
                                
                                self.controller.rotate_right ^= true;
                                if self.controller.rotate_right {
                                    exec.1.pop_front();
                                }
                            }
                            Some(Input::Ccw) => {
                                self.controller.left = false;
                                self.controller.right = false;
                                self.controller.rotate_right = false;
                                self.controller.soft_drop = false;
                                
                                self.controller.rotate_left ^= true;
                                if self.controller.rotate_left {
                                    exec.1.pop_front();
                                }
                            }
                        }
                    }
                }
                Event::PiecePlaced { piece, .. } => {
                    self.controller = Default::default();
                    if let Some(ref exec) = self.executing {
                        if exec.2 != *piece {
                            reset_bot = true;
                            eprintln!("Misdrop!");
                        }
                    } else {
                        reset_bot = true;
                    }
                    self.executing = None;
                }
                Event::GarbageAdded(_) => reset_bot = true,
                _ => {}
            }
        }
        if reset_bot {
            self.interface.reset(
                board.get_field(),
                board.b2b_bonus,
                board.combo
            );
        }
        update_info
    }
}