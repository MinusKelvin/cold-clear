use game_util::prelude::*;
use std::collections::VecDeque;
use libtetris::*;
use battle::{ PlayerUpdate, Event };
use arrayvec::ArrayVec;
use crate::res::Resources;

pub struct PlayerDrawState {
    board: ArrayVec<[ColoredRow; 40]>,
    state: State,
    statistics: Statistics,
    garbage_queue: u32,
    dead: bool,
    hold_piece: Option<Piece>,
    next_queue: VecDeque<Piece>,
    game_time: u32,
    combo_splash: Option<(u32, u32)>,
    back_to_back_splash: Option<u32>,
    clear_splash: Option<(&'static str, u32)>,
    name: String,
    info: Option<cold_clear::Info>
}

enum State {
    Falling(FallingPiece, FallingPiece),
    LineClearAnimation(ArrayVec<[i32; 4]>, i32),
    Delay
}

impl PlayerDrawState {
    pub fn new(queue: impl IntoIterator<Item=Piece>, name: String) -> Self {
        PlayerDrawState {
            board: ArrayVec::from([*ColoredRow::EMPTY; 40]),
            state: State::Delay,
            statistics: Statistics::default(),
            garbage_queue: 0,
            dead: false,
            hold_piece: None,
            next_queue: queue.into_iter().collect(),
            game_time: 0,
            combo_splash: None,
            back_to_back_splash: None,
            clear_splash: None,
            name,
            info: None
        }
    }

    pub fn update(
        &mut self, update: PlayerUpdate, info_update: Option<cold_clear::Info>, time: u32
    ) {
        self.garbage_queue = update.garbage_queue;
        self.info = info_update.or(self.info.take());
        self.game_time = time;
        if let State::LineClearAnimation(_, ref mut frames) = self.state {
            *frames += 1;
        }
        if let Some((_, timer)) = &mut self.combo_splash {
            if *timer == 0 {
                self.combo_splash = None;
            } else {
                *timer -= 1;
            }
        }
        if let Some(timer) = &mut self.back_to_back_splash {
            if *timer == 0 {
                self.back_to_back_splash = None;
            } else {
                *timer -= 1;
            }
        }
        if let Some((_, timer)) = &mut self.clear_splash {
            if *timer == 0 {
                self.clear_splash = None;
            } else {
                *timer -= 1;
            }
        }
        for event in &update.events {
            match event {
                Event::PiecePlaced { piece, locked, hard_drop_distance } => {
                    self.statistics.update(&locked);
                    for &(x, y, _) in &piece.cells() {
                        self.board[y as usize].set(x as usize, piece.kind.0.color());
                    }
                    if locked.cleared_lines.is_empty() {
                        self.state = State::Delay;
                    } else {
                        self.state = State::LineClearAnimation(locked.cleared_lines.clone(), 0);
                    }
                    if locked.b2b {
                        self.back_to_back_splash = Some(75);
                    }
                    let combo = locked.combo.unwrap_or(0);
                    if combo > 0 {
                        self.combo_splash = Some((combo, 75));
                    }
                    if locked.perfect_clear {
                        self.clear_splash = Some(("Perfect Clear", 135));
                        self.back_to_back_splash = None;
                    } else if locked.placement_kind.is_hard() {
                        self.clear_splash = Some((locked.placement_kind.name(), 75));
                    }
                }
                Event::PieceHeld(piece) => {
                    self.hold_piece = Some(*piece);
                    self.state = State::Delay;
                }
                Event::PieceSpawned { new_in_queue } => {
                    self.next_queue.push_back(*new_in_queue);
                    self.next_queue.pop_front();
                }
                Event::PieceFalling(piece, ghost) => {
                    self.state = State::Falling(*piece, *ghost);
                }
                Event::EndOfLineClearDelay => {
                    self.state = State::Delay;
                    self.board.retain(|row| !row.is_full());
                    while !self.board.is_full() {
                        self.board.push(*ColoredRow::EMPTY);
                    }
                }
                Event::GarbageAdded(columns) => {
                    self.board.truncate(40 - columns.len());
                    for &col in columns {
                        let mut row = *ColoredRow::EMPTY;
                        for x in 0..10 {
                            if x != col {
                                row.set(x, CellColor::Garbage);
                            }
                        }
                        self.board.insert(0, row);
                    }
                }
                Event::GameOver => self.dead = true,
                _ => {}
            }
        }
    }

    pub fn draw(&self, res: &mut Resources, offset_x: f32) {
        // Draw the playfield
        for y in 0..21 {
            for x in 0..10 {
                let cell_color = self.board[y].cell_color(x);
                res.sprite_batch.draw(
                    if cell_color == CellColor::Empty {
                        &res.sprites.blank
                    } else {
                        &res.sprites.filled
                    },
                    point2(offset_x + x as f32 + 3.0, y as f32 + 2.75),
                    cell_color_to_color(if self.dead && cell_color != CellColor::Empty {
                        CellColor::Unclearable
                    } else {
                        cell_color
                    })
                );
            }
        }
    }
}

fn cell_color_to_color(cell_color: CellColor) -> [u8; 4] {
    match cell_color {
        CellColor::Empty => [0xFF, 0xFF, 0xFF, 0xFF],
        CellColor::Garbage => [160, 160, 160, 0xFF],
        CellColor::Unclearable => [64, 64, 64, 0xFF],
        CellColor::Z => [255, 32, 32, 0xFF],
        CellColor::S => [32, 255, 32, 0xFF],
        CellColor::O => [255, 255, 32, 0xFF],
        CellColor::L => [255, 143, 32, 0xFF],
        CellColor::J => [96, 96, 255, 0xFF],
        CellColor::I => [32, 255, 255, 0xFF],
        CellColor::T => [143, 32, 255, 0xFF]
    }
}