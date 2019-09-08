use rand::prelude::*;
use std::collections::VecDeque;
use std::sync::mpsc::{ Sender, Receiver, TryRecvError, channel };
use arrayvec::ArrayVec;

mod display;
pub mod evaluation;
mod moves;
mod misa;
mod tree;

use libtetris::*;
use crate::tree::Tree;
use crate::moves::Input;
use crate::evaluation::Evaluator;

pub struct BotController {
    executing: Option<(bool, VecDeque<moves::Input>, FallingPiece)>,
    send: Sender<BotMsg>,
    recv: Receiver<BotResult>,
    controller: Controller,
    dead: bool
}

impl BotController {
    pub fn new(
        initial_board: Board, use_misa: bool, evaluator: impl Evaluator + Send + 'static
    ) -> Self {
        let (bot_send, recv) = channel();
        let (send, bot_recv) = channel();
        std::thread::spawn(move || {
            if use_misa {
                misa::glue(bot_recv, bot_send, initial_board);
            } else {
                use crate::evaluation::*;
                run(bot_recv, bot_send, initial_board, evaluator);
            }
        });

        BotController {
            executing: None,
            send, recv,
            controller: Controller::default(),
            dead: false
        }
    }

    pub fn controller(&mut self) -> Controller {
        self.controller
    }

    pub fn update(
        &mut self, events: &[Event], board: &Board<ColoredRow>
    ) -> Option<Info> {
        if self.dead {
            self.controller.hard_drop ^= true;
        }
        let mut update_info = None;
        match self.recv.try_recv() {
            Ok(BotResult::Move { inputs, expected_location, hold }) =>
                self.executing = Some((hold, inputs.into_iter().collect(), expected_location)),
            Ok(BotResult::BotInfo(lines)) => update_info = Some(lines),
            _ => {}
        }
        let mut reset_bot = false;
        for event in events {
            match event {
                Event::PieceSpawned { new_in_queue } => {
                    self.dead = self.send.send(BotMsg::NewPiece(*new_in_queue)).is_err();
                    if self.executing.is_none() {
                        self.dead = self.send.send(BotMsg::NextMove).is_err();
                    }
                }
                Event::SpawnDelayStart => if self.executing.is_none() {
                    self.dead = self.send.send(BotMsg::PrepareNextMove).is_err();
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
                            println!("Misdrop!");
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
            self.dead = self.send.send(BotMsg::Reset(board.to_compressed())).is_err();
        }
        update_info
    }
}

#[derive(Debug)]
enum BotMsg {
    Reset(Board),
    NewPiece(Piece),
    NextMove,
    PrepareNextMove
}

#[derive(Debug)]
enum BotResult {
    Move {
        inputs: moves::InputList,
        expected_location: FallingPiece,
        hold: bool
    },
    BotInfo(Info)
}

fn run(
    recv: Receiver<BotMsg>,
    send: Sender<BotResult>,
    board: Board,
    mut evaluator: impl Evaluator
) {
    const MOVEMENT_MODE: crate::moves::MovementMode = crate::moves::MovementMode::ZeroG;

    send.send(BotResult::BotInfo({
        let mut info = evaluator.info();
        info.insert(0, ("Cold Clear".to_string(), None));
        info
    })).ok();

    let mut tree = Tree::new(
        board,
        &Default::default(),
        false,
        &mut evaluator
    );

    let mut do_move = false;
    const THINK_OUTSIDE_SPAWN_DELAY: bool = true;
    let mut think = THINK_OUTSIDE_SPAWN_DELAY;
    loop {
        let result = if think {
            recv.try_recv()
        } else {
            recv.recv().map_err(|_| TryRecvError::Disconnected)
        };
        match result {
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => return,
            Ok(BotMsg::NewPiece(piece)) => if tree.add_next_piece(piece) {
                // Only death is possible
                break
            }
            Ok(BotMsg::Reset(board)) => {
                tree = Tree::new(
                    board,
                    &Default::default(),
                    false,
                    &mut evaluator
                );
            }
            Ok(BotMsg::NextMove) => do_move = true,
            Ok(BotMsg::PrepareNextMove) => {
                think = true;
            }
        }

        if do_move {
            let moves_considered = tree.child_nodes;
            match tree.into_best_child() {
                Ok(child) => {
                    do_move = false;
                    think = THINK_OUTSIDE_SPAWN_DELAY;
                    if send.send(BotResult::Move {
                        hold: child.hold,
                        inputs: child.mv.inputs,
                        expected_location: child.mv.location
                    }).is_err() {
                        return
                    }
                    if send.send(BotResult::BotInfo({
                        let mut info = evaluator.info();
                        info.insert(0, ("Cold Clear".to_owned(), None));
                        info.push(("Depth".to_owned(), Some(format!("{}", child.tree.depth))));
                        info.push(("Evaluation".to_owned(), Some("".to_owned())));
                        info.push(("".to_owned(), Some(format!("{}", child.tree.evaluation))));
                        info.push(("Nodes".to_owned(), Some("".to_owned())));
                        info.push(("".to_owned(), Some(format!("{}", moves_considered))));
                        info
                    })).is_err() {
                        return
                    }
                    tree = child.tree;
                }
                Err(t) => tree = t
            }
        }

        if think && tree.extend(MOVEMENT_MODE, &mut evaluator) {
            break
        }
    }
}
