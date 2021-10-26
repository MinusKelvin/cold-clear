use std::collections::VecDeque;

use arrayvec::ArrayVec;
use battle::{Event, PlayerUpdate};
use game_util::prelude::*;
use game_util::text::Alignment;
use libtetris::*;

use crate::res::Resources;

pub struct PlayerDrawState {
    board: ArrayVec<[ColoredRow; 40]>,
    state: State,
    statistics: Statistics,
    garbage_queue: u32,
    dead: bool,
    show_plan: bool,
    hold_piece: Option<Piece>,
    next_queue: VecDeque<Piece>,
    game_time: u32,
    combo_splash: Option<(u32, u32)>,
    back_to_back_splash: Option<u32>,
    clear_splash: Option<(&'static str, u32)>,
    name: String,
    info: Option<cold_clear::Info>,
}

enum State {
    Falling(FallingPiece, FallingPiece),
    LineClearAnimation(ArrayVec<[i32; 4]>, i32),
    Delay,
}

impl PlayerDrawState {
    pub fn new(queue: impl IntoIterator<Item = Piece>, name: String, show_plan: bool) -> Self {
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
            info: None,
            show_plan,
        }
    }

    pub fn update(
        &mut self,
        update: PlayerUpdate,
        info_update: Option<cold_clear::Info>,
        time: u32,
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
                Event::PiecePlaced { piece, locked, .. } => {
                    self.statistics.update(&locked);
                    for &(x, y) in &piece.cells() {
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
                    point2(offset_x + x as f32 + 4.0, y as f32 + 3.25),
                    cell_color_to_color(if self.dead && cell_color != CellColor::Empty {
                        CellColor::Unclearable
                    } else {
                        cell_color
                    }),
                );
            }
        }

        // Draw either the falling piece or the line clear animation
        match self.state {
            State::Falling(piece, ghost) => {
                for &(x, y) in &ghost.cells() {
                    res.sprite_batch.draw(
                        &res.sprites.ghost,
                        point2(offset_x + x as f32 + 4.0, y as f32 + 3.25),
                        cell_color_to_color(piece.kind.0.color()),
                    );
                }
                for &(x, y) in &piece.cells() {
                    res.sprite_batch.draw(
                        &res.sprites.filled,
                        point2(offset_x + x as f32 + 4.0, y as f32 + 3.25),
                        cell_color_to_color(piece.kind.0.color()),
                    );
                }
            }
            State::LineClearAnimation(ref lines, frame) => {
                let frame = (frame as usize).min(res.sprites.line_clear.len() - 1);
                for &y in lines {
                    res.sprite_batch.draw(
                        &res.sprites.line_clear[frame],
                        point2(offset_x + 8.5, y as f32 + 3.25),
                        [0xFF; 4],
                    );
                }
            }
            _ => {}
        }

        // Draw pending garbage bar
        for y in 0..self.garbage_queue {
            let w = res.sprites.garbage_bar.real_size.width / res.sprite_batch.pixels_per_unit;
            res.sprite_batch.draw(
                &res.sprites.garbage_bar,
                point2(offset_x + 13.5 + w / 2.0, y as f32 + 3.25),
                [0xFF; 4],
            );
        }

        // Draw hold piece and next queue
        res.text.draw_text(
            "Hold",
            offset_x + 2.0,
            21.85,
            Alignment::Center,
            [0xFF; 4],
            0.7,
            0,
        );
        if let Some(piece) = self.hold_piece {
            res.sprite_batch.draw(
                &res.sprites.piece[piece as usize],
                point2(offset_x + 2.0, 20.75),
                cell_color_to_color(piece.color()),
            );
        }
        res.text.draw_text(
            "Next",
            offset_x + 15.0,
            21.85,
            Alignment::Center,
            [0xFF; 4],
            0.7,
            0,
        );
        for (i, &piece) in self.next_queue.iter().enumerate() {
            res.sprite_batch.draw(
                &res.sprites.piece[piece as usize],
                point2(offset_x + 15.0, 20.75 - 2.0 * i as f32),
                cell_color_to_color(piece.color()),
            );
        }

        // Draw statistics
        res.text.draw_text(
            "Statistics",
            offset_x + 1.75,
            19.1,
            Alignment::Center,
            [0xFF; 4],
            0.6,
            0,
        );
        let seconds = self.game_time as f32 / 60.0;
        let mut lines = vec![
            ("Pieces", format!("{}", self.statistics.pieces)),
            (
                "PPS",
                format!("{:.1}", self.statistics.pieces as f32 / seconds),
            ),
            ("Lines", format!("{}", self.statistics.lines)),
            ("Attack", format!("{}", self.statistics.attack)),
            (
                "APM",
                format!("{:.1}", self.statistics.attack as f32 / seconds * 60.0),
            ),
            (
                "APP",
                format!(
                    "{:.3}",
                    self.statistics.attack as f32 / self.statistics.pieces as f32
                ),
            ),
            ("Max Ren", format!("{}", self.statistics.max_combo)),
            ("Single", format!("{}", self.statistics.singles)),
            ("Double", format!("{}", self.statistics.doubles)),
            ("Triple", format!("{}", self.statistics.triples)),
            ("Tetris", format!("{}", self.statistics.tetrises)),
            // ("Mini T0", format!("{}", self.statistics.mini_tspin_zeros)),
            // ("Mini T1", format!("{}", self.statistics.mini_tspin_singles)),
            // ("Mini T2", format!("{}", self.statistics.mini_tspin_doubles)),
            ("T-Spin 0", format!("{}", self.statistics.tspin_zeros)),
            ("T-Spin 1", format!("{}", self.statistics.tspin_singles)),
            ("T-Spin 2", format!("{}", self.statistics.tspin_doubles)),
            ("T-Spin 3", format!("{}", self.statistics.tspin_triples)),
            ("Perfect", format!("{}", self.statistics.perfect_clears)),
        ];
        if let Some(ref info) = self.info {
            // Bot info
            lines.push(("", "".to_owned()));
            match info {
                cold_clear::Info::Normal(info) => {
                    lines.push(("Freestyle", "".to_owned()));
                    lines.push(("Depth", format!("{}", info.depth)));
                    lines.push(("Nodes", format!("{}", info.nodes)));
                    lines.push(("O. Rank", format!("{}", info.original_rank)));
                }
                cold_clear::Info::Book => {
                    lines.push(("Book", "".to_owned()));
                }
                cold_clear::Info::PcLoop(info) => {
                    lines.push(("PC Loop", "".to_owned()));
                    #[cfg(not(target_arch = "wasm32"))]
                    lines.push(("Depth", format!("{}", info.depth)));
                }
            }
        }
        let mut labels = String::new();
        let mut values = String::new();
        for (label, value) in lines {
            labels.push_str(label);
            labels.push('\n');
            values.push_str(&value);
            values.push('\n');
        }
        res.text.draw_text(
            &labels,
            offset_x + 0.2,
            18.4,
            Alignment::Left,
            [0xFF; 4],
            0.45,
            0,
        );
        res.text.draw_text(
            &values,
            offset_x + 3.3,
            18.4,
            Alignment::Right,
            [0xFF; 4],
            0.45,
            0,
        );

        if self.show_plan {
            if let Some(ref info) = self.info {
                let mut has_pc = false;
                for (_, l) in info.plan() {
                    if l.perfect_clear {
                        has_pc = true;
                    }
                }

                // Draw bot plan
                let mut y_map = [0; 40];
                for i in 0..40 {
                    y_map[i] = i as i32;
                }
                for (placement, lock) in info.plan() {
                    for &(x, y, d) in &placement.cells_with_connections() {
                        res.sprite_batch.draw(
                            &res.sprites.plan[d.as_usize()],
                            point2(offset_x + x as f32 + 4.0, y_map[y as usize] as f32 + 3.25),
                            cell_color_to_color(placement.kind.0.color()),
                        );
                    }
                    let mut new_map = [0; 40];
                    let mut j = 0;
                    for i in 0..40 {
                        if !lock.cleared_lines.contains(&i) {
                            new_map[j] = y_map[i as usize];
                            j += 1;
                        }
                    }
                    y_map = new_map;

                    if !has_pc && lock.placement_kind.is_hard() && lock.placement_kind.is_clear()
                        || lock.perfect_clear
                    {
                        break;
                    }
                }
            }
        }

        // Draw player name
        res.text.draw_text(
            &self.name,
            offset_x + 15.0,
            21.0 - 2.0 * self.next_queue.len() as f32,
            Alignment::Center,
            [0xFF; 4],
            0.45,
            0,
        );

        // Draw clear info stuff
        if let Some(timer) = self.back_to_back_splash {
            res.text.draw_text(
                "Back-To-Back",
                offset_x + 4.0,
                1.65,
                Alignment::Left,
                [0xFF, 0xFF, 0xFF, (timer.min(15) * 0xFF / 15) as u8],
                0.75,
                0,
            );
        }
        if let Some((combo, timer)) = self.combo_splash {
            res.text.draw_text(
                &format!("{} Combo", combo),
                offset_x + 13.0,
                1.65,
                Alignment::Right,
                [0xFF, 0xFF, 0xFF, (timer.min(15) * 0xFF / 15) as u8],
                0.75,
                0,
            );
        }
        if let Some((txt, timer)) = self.clear_splash {
            res.text.draw_text(
                txt,
                offset_x + 8.5,
                0.65,
                Alignment::Center,
                [0xFF, 0xFF, 0xFF, (timer.min(15) * 0xFF / 15) as u8],
                0.75,
                0,
            );
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
        CellColor::T => [143, 32, 255, 0xFF],
    }
}
