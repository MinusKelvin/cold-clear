use rand::prelude::*;
use std::collections::VecDeque;
use std::sync::mpsc::{ Sender, Receiver, TryRecvError, channel };
use arrayvec::ArrayVec;

mod display;
mod evaluation;
mod moves;
mod misa;
mod tree;

use libtetris::*;
use crate::tree::Tree;
use crate::moves::Input;

pub struct BotController {
    executing: Option<(bool, VecDeque<moves::Input>, FallingPiece)>,
    send: Sender<BotMsg>,
    recv: Receiver<BotResult>,
    controller: Controller,
    dead: bool
}

impl BotController {
    pub fn new(queue: impl IntoIterator<Item=Piece>, use_misa: bool) -> Self {
        let (bot_send, recv) = channel();
        let (send, bot_recv) = channel();
        std::thread::spawn(move || {
            if use_misa {
                misa::glue(bot_recv, bot_send);
            } else {
                run(bot_recv, bot_send);
            }
        });

        for piece in queue {
            send.send(BotMsg::NewPiece(piece)).unwrap();
        }

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
    ) -> Option<Vec<(String, Option<String>)>> {
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
    BotInfo(Vec<(String, Option<String>)>)
}

fn run(recv: Receiver<BotMsg>, send: Sender<BotResult>) {
    let transient_weights = evaluation::BoardWeights {
        back_to_back: 50,
        bumpiness: -10,
        bumpiness_sq: -5,
        height: -40,
        top_half: -150,
        top_quarter: -500,
        cavity_cells: -150,
        cavity_cells_sq: -10,
        overhang_cells: -50,
        overhang_cells_sq: -10,
        covered_cells: -10,
        covered_cells_sq: -10,
        tslot_present: 150,
        well_depth: 50,
        max_well_depth: 8
    };

    let acc_weights = evaluation::PlacementWeights {
        soft_drop: -10,
        b2b_clear: 100,
        clear1: -150,
        clear2: -100,
        clear3: -50,
        clear4: 400,
        tspin1: 130,
        tspin2: 400,
        tspin3: 600,
        mini_tspin1: 0,
        mini_tspin2: 100,
        perfect_clear: 1000,
        combo_table: libtetris::COMBO_GARBAGE.iter()
            .map(|&v| v as i32 * 100)
            .collect::<ArrayVec<[_; 12]>>()
            .into_inner()
            .unwrap()
    };

    const MOVEMENT_MODE: crate::moves::MovementMode = crate::moves::MovementMode::ZeroG;

    let mut tree = Tree::new(
        Board::new(),
        &Default::default(),
        false,
        &transient_weights,
        &acc_weights
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
            Ok(BotMsg::NewPiece(piece)) => tree.add_next_piece(piece),
            Ok(BotMsg::Reset(board)) => {
                tree = Tree::new(
                    board,
                    &Default::default(),
                    false,
                    &transient_weights,
                    &acc_weights
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
                    if send.send(BotResult::BotInfo(vec![
                        ("Cold Clear".to_owned(), None),
                        ("'Naive'".to_owned(), None),
                        ("Depth".to_owned(), Some(format!("{}", child.tree.depth))),
                        ("Evaluation".to_owned(), Some("".to_owned())),
                        ("".to_owned(), Some(format!("{}", child.tree.evaluation))),
                        ("Nodes".to_owned(), Some("".to_owned())),
                        ("".to_owned(), Some(format!("{}", moves_considered))),
                    ])).is_err() {
                        return
                    }
                    tree = child.tree;
                }
                Err(t) => tree = t
            }
        }

        if think && tree.extend(MOVEMENT_MODE, &transient_weights, &acc_weights) {
            break
        }
    }
}
