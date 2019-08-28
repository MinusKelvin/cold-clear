use crate::*;
use rand::prelude::*;

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

pub struct Game {
    pub board: Board<ColoredRow, Statistics>,
    pub state: GameState,
    config: GameConfig,
    did_hold: bool,
    prev: Controller,
    used: Controller,
    das_delay: u32,
    pub garbage_queue: u32,
    attacking: u32
}

/// Units are in ticks
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct GameConfig {
    pub spawn_delay: u32,
    pub line_clear_delay: u32,
    pub delayed_auto_shift: u32,
    pub auto_repeat_rate: u32,
    pub soft_drop_speed: u32,
    /// Measured in 1/100 of a tick
    pub gravity: i32,
    pub next_queue_size: u32,
    pub margin_time: Option<u32>,
    pub max_garbage_add: u32
}

pub enum Event {
    PieceSpawned { new_in_queue: Piece },
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
        hard_drop_distance: Option<i32>
    },
    GarbageSent(u32),
    GarbageAdded(Vec<usize>),
    GameOver
}

impl Game {
    pub fn new(config: GameConfig) -> Self {
        let mut board = Board::new();
        for _ in 0..config.next_queue_size {
            board.add_next_piece(board.generate_next_piece());
        }
        Game {
            board, config,
            prev: Default::default(),
            used: Default::default(),
            did_hold: false,
            das_delay: config.delayed_auto_shift,
            state: GameState::SpawnDelay(config.next_queue_size),
            garbage_queue: 0,
            attacking: 0
        }
    }

    pub fn update(&mut self, current: Controller) -> Vec<Event> {
        update_input(&mut self.used.left, self.prev.left, current.left);
        update_input(&mut self.used.right, self.prev.right, current.right);
        update_input(&mut self.used.rotate_right, self.prev.rotate_right, current.rotate_right);
        update_input(&mut self.used.rotate_left, self.prev.rotate_left, current.rotate_left);
        update_input(&mut self.used.soft_drop, self.prev.soft_drop, current.soft_drop);
        update_input(&mut self.used.hold, self.prev.hold, current.hold);
        self.used.hard_drop = !self.prev.hard_drop && current.hard_drop;
        self.used.soft_drop = current.soft_drop;

        if current.left != current.right && self.prev.left == current.left {
            if self.used.left || self.used.right {
                // While movement is buffered, don't let the time
                // until the next shift fall below the auto-repeat rate.
                // Otherwise we might rapidly shift twice when a piece spawns.
                if self.das_delay > self.config.auto_repeat_rate {
                    self.das_delay -= 1;
                }
            } else if self.das_delay == 0 {
                // Apply auto-shift
                self.das_delay = self.config.auto_repeat_rate;
                self.used.left = current.left;
                self.used.right = current.right;
            } else {
                self.das_delay -= 1;
            }
        } else {
            // Reset delayed auto shift
            self.das_delay = self.config.delayed_auto_shift;
        }

        self.prev = current;

        match self.state {
            GameState::SpawnDelay(0) => {
                let next_piece = self.board.advance_queue().unwrap();
                let new_piece = self.board.generate_next_piece();
                self.board.add_next_piece(new_piece);
                if let Some(spawned) = FallingPiece::spawn(next_piece, &self.board) {
                    self.state = GameState::Falling(FallingState {
                        piece: spawned,
                        lowest_y: spawned.cells().into_iter().map(|(_,y)| y).min().unwrap(),
                        rotation_move_count: 0,
                        gravity: self.config.gravity,
                        lock_delay: 30,
                        soft_drop_delay: 0
                    });
                    let mut ghost = spawned;
                    ghost.sonic_drop(&self.board);
                    vec![
                        Event::PieceSpawned { new_in_queue: new_piece },
                        Event::PieceFalling(spawned, ghost)
                    ]
                } else {
                    self.state = GameState::GameOver;
                    vec![Event::GameOver]
                }
            }
            GameState::SpawnDelay(ref mut delay) => {
                *delay -= 1;
                vec![]
            }
            GameState::LineClearDelay(0) => {
                self.state = GameState::SpawnDelay(self.config.spawn_delay);
                let mut events = vec![Event::EndOfLineClearDelay];
                self.deal_garbage(&mut events);
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
                        if let Some(spawned) = FallingPiece::spawn(piece, &self.board) {
                            *falling = FallingState {
                                piece: spawned,
                                lowest_y: spawned.cells().into_iter().map(|(_,y)| y).min().unwrap(),
                                rotation_move_count: 0,
                                gravity: self.config.gravity,
                                lock_delay: 30,
                                soft_drop_delay: 0
                            };
                            let mut ghost = spawned;
                            ghost.sonic_drop(&self.board);
                            events.push(Event::PieceFalling(spawned, ghost));
                        } else {
                            self.state = GameState::GameOver;
                            events.push(Event::GameOver);
                        }
                    } else {
                        self.state = GameState::SpawnDelay(self.config.spawn_delay);
                    }
                    return events;
                }

                // Rotate
                if self.used.rotate_right {
                    if falling.piece.cw(&self.board) {
                        self.used.rotate_right = false;
                        falling.rotation_move_count += 1;
                        falling.lock_delay = 30;
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
                        falling.lock_delay = 30;
                        if falling.piece.tspin != TspinStatus::None {
                            events.push(Event::PieceTSpined);
                        } else {
                            events.push(Event::PieceRotated);
                        }
                    }
                }

                // Shift
                if self.used.left {
                    if falling.piece.shift(&self.board, -1, 0) {
                        self.used.left = false;
                        falling.rotation_move_count += 1;
                        falling.lock_delay = 30;
                        events.push(Event::PieceMoved);
                    }
                }
                if self.used.right {
                    if falling.piece.shift(&self.board, 1, 0) {
                        self.used.right = false;
                        falling.rotation_move_count += 1;
                        falling.lock_delay = 30;
                        events.push(Event::PieceMoved);
                    }
                }

                // 15 moves reset
                let low_y = falling.piece.cells().into_iter().map(|(_,y)| y).min().unwrap();
                if low_y < falling.lowest_y {
                    falling.rotation_move_count = 0;
                    falling.lowest_y = low_y;
                }

                // 15 moves lock rule
                if falling.rotation_move_count >= 15 {
                    let mut p = falling.piece;
                    p.sonic_drop(&self.board);
                    let low_y = p.cells().into_iter().map(|(_,y)| y).min().unwrap();
                    if low_y >= falling.lowest_y {
                        let f = *falling;
                        self.lock(f, &mut events, None);
                        return events;
                    }
                }

                // Hard drop
                if self.used.hard_drop {
                    let y = falling.piece.y;
                    falling.piece.sonic_drop(&self.board);
                    let distance = y - falling.piece.y;
                    let f = *falling;
                    self.lock(f, &mut events, Some(distance));
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
                        self.lock(f, &mut events, None);
                        return events;
                    }
                } else {
                    // Gravity
                    falling.lock_delay = 30;
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
                            if falling.soft_drop_delay == 0 {
                                falling.piece.shift(&self.board, 0, -1);
                                falling.soft_drop_delay = self.config.soft_drop_speed;
                                falling.gravity = self.config.gravity;
                                events.push(Event::PieceMoved);
                                if self.board.on_stack(&falling.piece) {
                                    events.push(Event::StackTouched);
                                }
                                events.push(Event::SoftDropped);
                            } else {
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

    fn lock(&mut self, falling: FallingState, events: &mut Vec<Event>, dist: Option<i32>) {
        self.did_hold = false;
        let locked = self.board.lock_piece(falling.piece);;

        events.push(Event::PiecePlaced {
            piece: falling.piece,
            locked: locked.clone(),
            hard_drop_distance: dist
        });

        if locked.locked_out {
            self.state = GameState::GameOver;
            events.push(Event::GameOver);
        } else if locked.cleared_lines.is_empty() {
            self.state = GameState::SpawnDelay(self.config.spawn_delay);
            self.deal_garbage(events);
        } else {
            self.attacking += locked.garbage_sent;
            self.state = GameState::LineClearDelay(self.config.line_clear_delay);
        }
    }

    fn deal_garbage(&mut self, events: &mut Vec<Event>) {
        if self.attacking > self.garbage_queue {
            self.attacking -= self.garbage_queue;
            self.garbage_queue = 0;
        } else {
            self.garbage_queue -= self.attacking;
            self.attacking = 0;
        }
        if self.garbage_queue > 0 {
            let mut dead = false;
            let mut col = thread_rng().gen_range(0, 10);
            let mut garbage_columns = vec![];
            for _ in 0..self.garbage_queue.min(self.config.max_garbage_add) {
                if thread_rng().gen_bool(1.0/3.0) {
                    col = thread_rng().gen_range(0, 10);
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

pub struct Battle {
    pub player_1: Game,
    pub player_2: Game,
    pub time: u32,
    margin_time: Option<u32>,
    multiplier: f32
}

impl Battle {
    pub fn new(config: GameConfig) -> Self {
        Battle {
            player_1: Game::new(config),
            player_2: Game::new(config),
            time: 0,
            margin_time: config.margin_time,
            multiplier: 1.0
        }
    }

    pub fn update(&mut self, p1: Controller, p2: Controller) -> (Vec<Event>, Vec<Event>) {
        self.time += 1;
        if let Some(margin_time) = self.margin_time {
            if self.time > margin_time {
                self.multiplier += 1.0 / 1800.0;
            }
        }

        let p1_events = self.player_1.update(p1);
        let p2_events = self.player_2.update(p2);

        for event in &p1_events {
            if let &Event::GarbageSent(amt) = event {
                self.player_2.garbage_queue += (amt as f32 * self.multiplier) as u32;
            }
        }
        for event in &p2_events {
            if let &Event::GarbageSent(amt) = event {
                self.player_1.garbage_queue += (amt as f32 * self.multiplier) as u32;
            }
        }

        (p1_events, p2_events)
    }
}

pub enum GameState {
    SpawnDelay(u32),
    LineClearDelay(u32),
    Falling(FallingState),
    GameOver
}

#[derive(Copy, Clone, Debug)]
pub struct FallingState {
    piece: FallingPiece,
    lowest_y: i32,
    rotation_move_count: u32,
    gravity: i32,
    lock_delay: u32,
    soft_drop_delay: u32
}

impl Default for GameConfig {
    fn default() -> Self {
        // Use something approximating Puyo Puyo Tetris
        GameConfig {
            spawn_delay: 7,
            line_clear_delay: 45,
            delayed_auto_shift: 12,
            auto_repeat_rate: 1,
            soft_drop_speed: 1,
            next_queue_size: 5,
            gravity: 4500,
            margin_time: Some(7200),
            max_garbage_add: 10
        }
    }
}