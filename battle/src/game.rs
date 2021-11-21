use libtetris::*;
use rand::prelude::*;
use serde::{Deserialize, Serialize};

use crate::GameConfig;

pub struct Game {
    pub board: Board<ColoredRow>,
    state: GameState,
    config: GameConfig,
    did_hold: bool,
    prev: Controller,
    used: Controller,
    left_das: u32,
    right_das: u32,
    going_right: bool,
    pub garbage_queue: u32,
    pub attacking: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Event {
    PieceSpawned {
        new_in_queue: Piece,
    },
    SpawnDelayStart,
    FrameBeforePieceSpawns,
    PieceMoved,
    PieceRotated,
    PieceTSpined,
    PieceHeld(Piece),
    StackTouched,
    SoftDropped,
    PieceFalling(FallingPiece, FallingPiece),
    EndOfLineClearDelay,
    PiecePlaced {
        piece: FallingPiece,
        locked: LockResult,
        hard_drop_distance: Option<i32>,
    },
    GarbageSent(u32),
    GarbageAdded(Vec<usize>),
    GameOver,
}

enum GameState {
    SpawnDelay(u32),
    LineClearDelay(u32),
    Falling(FallingState),
    GameOver,
}

#[derive(Copy, Clone, Debug)]
struct FallingState {
    piece: FallingPiece,
    lowest_y: i32,
    rotation_move_count: u32,
    gravity: i32,
    lock_delay: u32,
    soft_drop_delay: u32,
}

impl Game {
    pub fn new(config: GameConfig, piece_rng: &mut impl Rng) -> Self {
        let mut board = Board::new();
        for _ in 0..config.next_queue_size {
            board.add_next_piece(board.generate_next_piece(piece_rng));
        }
        Game {
            board,
            config,
            prev: Default::default(),
            used: Default::default(),
            did_hold: false,
            left_das: config.delayed_auto_shift,
            right_das: config.delayed_auto_shift,
            going_right: false,
            state: GameState::SpawnDelay(config.spawn_delay),
            garbage_queue: 0,
            attacking: 0,
        }
    }

    pub fn update(
        &mut self,
        current: Controller,
        piece_rng: &mut impl Rng,
        garbage_rng: &mut impl Rng,
    ) -> Vec<Event> {
        update_input(&mut self.used.left, self.prev.left, current.left);
        update_input(&mut self.used.right, self.prev.right, current.right);
        update_input(
            &mut self.used.rotate_right,
            self.prev.rotate_right,
            current.rotate_right,
        );
        update_input(
            &mut self.used.rotate_left,
            self.prev.rotate_left,
            current.rotate_left,
        );
        update_input(
            &mut self.used.soft_drop,
            self.prev.soft_drop,
            current.soft_drop,
        );
        update_input(&mut self.used.hold, self.prev.hold, current.hold);
        self.used.hard_drop = !self.prev.hard_drop && current.hard_drop;
        self.used.soft_drop = current.soft_drop;

        if !self.prev.left && current.left {
            self.going_right = false;
            self.used.right = false;
        } else if !self.prev.right && current.right {
            self.going_right = true;
            self.used.left = false;
        }

        if current.left {
            if self.used.left || current.right && self.going_right {
                if self.left_das > self.config.auto_repeat_rate {
                    self.left_das -= 1;
                } else {
                    self.left_das = self.config.auto_repeat_rate;
                }
            } else {
                if self.left_das != 0 {
                    self.left_das -= 1;
                }
                if self.left_das == 0 {
                    self.used.left = true;
                    self.left_das = self.config.auto_repeat_rate;
                }
            }
        } else {
            self.left_das = self.config.delayed_auto_shift;
        }

        if current.right {
            if self.used.right || current.left && !self.going_right {
                if self.right_das > self.config.auto_repeat_rate {
                    self.right_das -= 1;
                } else {
                    self.right_das = self.config.auto_repeat_rate;
                }
            } else {
                if self.right_das != 0 {
                    self.right_das -= 1;
                }
                if self.right_das == 0 {
                    self.used.right = true;
                    self.right_das = self.config.auto_repeat_rate;
                }
            }
        } else {
            self.right_das = self.config.delayed_auto_shift;
        }

        self.prev = current;

        match self.state {
            GameState::SpawnDelay(0) => {
                let mut events = vec![];
                if self.config.spawn_delay == 0 {
                    events.push(Event::FrameBeforePieceSpawns);
                }
                let new_piece = self.board.generate_next_piece(piece_rng);
                self.board.add_next_piece(new_piece);
                let next_piece = self.board.advance_queue().unwrap();
                if let Some(spawned) = SpawnRule::Row19Or20.spawn(next_piece, &self.board) {
                    self.state = GameState::Falling(FallingState {
                        piece: spawned,
                        lowest_y: spawned.cells().iter().map(|&(_, y)| y).min().unwrap(),
                        rotation_move_count: 0,
                        gravity: self.config.gravity,
                        lock_delay: 30,
                        soft_drop_delay: 0,
                    });
                    let mut ghost = spawned;
                    ghost.sonic_drop(&self.board);
                    events.push(Event::PieceSpawned {
                        new_in_queue: new_piece,
                    });
                    events.push(Event::PieceFalling(spawned, ghost));
                } else {
                    self.state = GameState::GameOver;
                    events.push(Event::GameOver);
                }
                events
            }
            GameState::SpawnDelay(ref mut delay) => {
                *delay -= 1;
                let mut events = vec![];
                if *delay == 0 {
                    events.push(Event::FrameBeforePieceSpawns);
                }
                if *delay + 1 == self.config.spawn_delay {
                    events.push(Event::SpawnDelayStart);
                }
                events
            }
            GameState::LineClearDelay(0) => {
                self.state = GameState::SpawnDelay(self.config.spawn_delay);
                let mut events = vec![Event::EndOfLineClearDelay];
                if !self.config.garbage_blocking {
                    self.deal_garbage(&mut events, garbage_rng);
                }
                events
            }
            GameState::LineClearDelay(ref mut delay) => {
                *delay -= 1;
                vec![]
            }
            GameState::GameOver => vec![Event::GameOver],
            GameState::Falling(ref mut falling) => {
                let mut events = vec![];
                let was_on_stack = self.board.on_stack(&falling.piece);

                // Hold
                if !self.did_hold && self.used.hold {
                    self.did_hold = true;
                    events.push(Event::PieceHeld(falling.piece.kind.0));
                    if let Some(piece) = self.board.hold(falling.piece.kind.0) {
                        // Piece in hold; the piece spawns instantly
                        if let Some(spawned) = SpawnRule::Row19Or20.spawn(piece, &self.board) {
                            *falling = FallingState {
                                piece: spawned,
                                lowest_y: spawned.cells().iter().map(|&(_, y)| y).min().unwrap(),
                                rotation_move_count: 0,
                                gravity: self.config.gravity,
                                lock_delay: 30,
                                soft_drop_delay: 0,
                            };
                            let mut ghost = spawned;
                            ghost.sonic_drop(&self.board);
                            events.push(Event::PieceFalling(spawned, ghost));
                        } else {
                            // Hold piece couldn't spawn; Block Out
                            self.state = GameState::GameOver;
                            events.push(Event::GameOver);
                        }
                    } else {
                        // Nothing in hold; spawn next piece normally
                        self.state = GameState::SpawnDelay(self.config.spawn_delay);
                    }
                    return events;
                }

                // Rotate
                if self.used.rotate_right {
                    if falling.piece.cw(&self.board) {
                        self.used.rotate_right = false;
                        falling.rotation_move_count += 1;
                        falling.lock_delay = self.config.lock_delay;
                        if falling.piece.tspin != TspinStatus::None {
                            events.push(Event::PieceTSpined);
                        } else {
                            events.push(Event::PieceRotated);
                        }
                    }
                }
                if self.used.rotate_left {
                    if falling.piece.ccw(&self.board) {
                        self.used.rotate_left = false;
                        falling.rotation_move_count += 1;
                        falling.lock_delay = self.config.lock_delay;
                        if falling.piece.tspin != TspinStatus::None {
                            events.push(Event::PieceTSpined);
                        } else {
                            events.push(Event::PieceRotated);
                        }
                    }
                }

                // Shift
                while self.used.left && falling.piece.shift(&self.board, -1, 0) {
                    self.used.left = self.config.auto_repeat_rate == 0 && self.left_das == 0;
                    falling.rotation_move_count += 1;
                    falling.lock_delay = self.config.lock_delay;
                    events.push(Event::PieceMoved);
                }
                while self.used.right && falling.piece.shift(&self.board, 1, 0) {
                    self.used.right = self.config.auto_repeat_rate == 0 && self.right_das == 0;
                    falling.rotation_move_count += 1;
                    falling.lock_delay = self.config.lock_delay;
                    events.push(Event::PieceMoved);
                }

                // 15 move lock rule reset
                let low_y = falling.piece.cells().iter().map(|&(_, y)| y).min().unwrap();
                if low_y < falling.lowest_y {
                    falling.rotation_move_count = 0;
                    falling.lowest_y = low_y;
                }

                // 15 move lock rule
                if falling.rotation_move_count >= self.config.move_lock_rule {
                    let mut p = falling.piece;
                    p.sonic_drop(&self.board);
                    let low_y = p.cells().iter().map(|&(_, y)| y).min().unwrap();
                    // I don't think the 15 move lock rule applies if the piece can fall to a lower
                    // y position than it has ever reached before.
                    if low_y >= falling.lowest_y {
                        let mut f = *falling;
                        f.piece = p;
                        self.lock(f, &mut events, garbage_rng, None);
                        return events;
                    }
                }

                // Hard drop
                if self.used.hard_drop {
                    let y = falling.piece.y;
                    falling.piece.sonic_drop(&self.board);
                    let distance = y - falling.piece.y;
                    let f = *falling;
                    self.lock(f, &mut events, garbage_rng, Some(distance));
                    return events;
                }

                if self.board.on_stack(&falling.piece) {
                    // Lock delay
                    if !was_on_stack {
                        events.push(Event::StackTouched);
                    }
                    falling.lock_delay -= 1;
                    falling.gravity = self.config.gravity;
                    if falling.lock_delay == 0 {
                        let f = *falling;
                        self.lock(f, &mut events, garbage_rng, None);
                        return events;
                    }
                } else {
                    // Gravity
                    falling.lock_delay = self.config.lock_delay;
                    falling.gravity -= 100;
                    while falling.gravity < 0 {
                        falling.gravity += self.config.gravity;
                        falling.piece.shift(&self.board, 0, -1);
                    }

                    if self.board.on_stack(&falling.piece) {
                        events.push(Event::StackTouched);
                    } else if self.config.gravity > self.config.soft_drop_speed as i32 * 100 {
                        // Soft drop
                        if self.used.soft_drop {
                            while falling.soft_drop_delay == 0 {
                                falling.piece.shift(&self.board, 0, -1);
                                falling.soft_drop_delay = self.config.soft_drop_speed;
                                falling.gravity = self.config.gravity;
                                events.push(Event::PieceMoved);
                                events.push(Event::SoftDropped);
                                if self.board.on_stack(&falling.piece) {
                                    events.push(Event::StackTouched);
                                    break;
                                }
                            }
                            if falling.soft_drop_delay != 0 {
                                falling.soft_drop_delay -= 1;
                            }
                        } else {
                            falling.soft_drop_delay = 0;
                        }
                    }
                }

                let mut ghost = falling.piece;
                ghost.sonic_drop(&self.board);
                events.push(Event::PieceFalling(falling.piece, ghost));

                events
            }
        }
    }

    fn lock(
        &mut self,
        falling: FallingState,
        events: &mut Vec<Event>,
        garbage_rng: &mut impl Rng,
        dist: Option<i32>,
    ) {
        self.did_hold = false;
        let locked = self.board.lock_piece(falling.piece);

        events.push(Event::PiecePlaced {
            piece: falling.piece,
            locked: locked.clone(),
            hard_drop_distance: dist,
        });

        if locked.locked_out {
            self.state = GameState::GameOver;
            events.push(Event::GameOver);
        } else if locked.cleared_lines.is_empty() {
            self.state = GameState::SpawnDelay(self.config.spawn_delay);
            self.deal_garbage(events, garbage_rng);
        } else {
            self.attacking += locked.garbage_sent;
            self.state = GameState::LineClearDelay(self.config.line_clear_delay);
        }
    }

    fn deal_garbage(&mut self, events: &mut Vec<Event>, rng: &mut impl Rng) {
        if self.attacking > self.garbage_queue {
            self.attacking -= self.garbage_queue;
            self.garbage_queue = 0;
        } else {
            self.garbage_queue -= self.attacking;
            self.attacking = 0;
        }
        if self.garbage_queue > 0 {
            let mut dead = false;
            let mut col = rng.gen_range(0, 10);
            let mut garbage_columns = vec![];
            for _ in 0..self.garbage_queue.min(self.config.max_garbage_add) {
                if rng.gen_bool(self.config.garbage_messiness.into_inner()) {
                    col = rng.gen_range(0, 10);
                }
                garbage_columns.push(col);
                dead |= self.board.add_garbage(col);
            }
            self.garbage_queue -= self.garbage_queue.min(self.config.max_garbage_add);
            events.push(Event::GarbageAdded(garbage_columns));
            if dead {
                events.push(Event::GameOver);
                self.state = GameState::GameOver;
            }
        } else if self.attacking > 0 {
            events.push(Event::GarbageSent(self.attacking));
            self.attacking = 0;
        }
    }
}

fn update_input(used: &mut bool, prev: bool, current: bool) {
    if !current {
        *used = false
    } else if !prev {
        *used = true;
    }
}
