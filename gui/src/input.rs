use libtetris::*;
use battle::{ Event, PieceMoveExecutor };

pub trait InputSource {
    fn controller(&mut self) -> Controller;
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
    fn controller(&mut self) -> Controller {
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