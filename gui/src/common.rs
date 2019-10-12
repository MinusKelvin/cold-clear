use ggez::{ Context, GameResult };
use ggez::graphics::*;
use ggez::graphics::spritebatch::SpriteBatch;
use std::collections::VecDeque;
use libtetris::*;
use battle::{ PlayerUpdate, Event };
use arrayvec::ArrayVec;
use crate::interface::text;
use rand::prelude::*;

pub struct BoardDrawState {
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
    hard_drop_particles: Option<(u32, Vec<(f32, f32, f32)>)>,
    info: Option<bot::Info>
}

enum State {
    Falling(FallingPiece, FallingPiece),
    LineClearAnimation(ArrayVec<[i32; 4]>, i32),
    Delay
}

impl BoardDrawState {
    pub fn new(queue: impl IntoIterator<Item=Piece>, name: String) -> Self {
        BoardDrawState {
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
            hard_drop_particles: None,
            name,
            info: None
        }
    }

    pub fn update(&mut self, update: &[Event], info_update: Option<bot::Info>, time: u32) {
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
        if let Some((timer, particles)) = &mut self.hard_drop_particles {
            if *timer == 0 {
                self.hard_drop_particles = None;
            } else {
                *timer -= 1;
                let t = *timer as f32 / 10.0;
                for (x, y, factor) in particles {
                    *x += *factor * t / 5.0;
                    *y += t*t * (1.0 - factor.abs());
                }
            }
        }
        for event in update {
            match event {
                Event::PiecePlaced { piece, locked, hard_drop_distance } => {
                    self.statistics.update(&locked);
                    if hard_drop_distance.is_some() {
                        let mut particles = vec![];
                        for (x, y, _) in piece.cells() {
                            if y == 0 || self.board[y as usize - 1].get(x as usize) {
                                for i in 0..5 {
                                    let r: f32 = thread_rng().gen();
                                    particles.push((x as f32+r, y as f32, r-0.5));
                                    let r = i as f32 / 4.0;
                                    particles.push((x as f32+r, y as f32, r-0.5));
                                }
                            }
                        }
                        self.hard_drop_particles = Some((5, particles));
                    }
                    for (x, y, _) in piece.cells() {
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
                    self.next_queue.pop_front();
                    self.next_queue.push_back(*new_in_queue);
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

    pub fn draw(
        &self,
        ctx: &mut Context,
        sprites: &mut SpriteBatch,
        particle_mesh: &mut MeshBuilder,
        text_x: f32,
        scale: f32
    ) -> GameResult {
        // Draw the playfield
        for y in 0..21 {
            for x in 0..10 {
                sprites.add(draw_tile(
                    x as i32+3, y as i32, if self.board[y].cell_color(x) == CellColor::Empty
                        { 0 } else { 1 },
                    0, cell_color_to_color({
                        let color = self.board[y].cell_color(x);
                        if self.dead && color != CellColor::Empty {
                            CellColor::Unclearable
                        } else {
                            color
                        }
                    })
                ));
            }
        }
        // Draw hard drop particle effects
        if let Some((timer, particles)) = &self.hard_drop_particles {
            for &(x, y, factor) in particles {
                particle_mesh.circle(
                    DrawMode::Fill(Default::default()),
                    [x+3.0,20.25-y],
                    (factor/20.0+0.05) * (*timer + 1) as f32 / 5.0,
                    0.1/scale,
                    WHITE
                );
            }
        }
        // Draw either the falling piece or the line clear animation
        match self.state {
            State::Falling(piece, ghost) => {
                for (x,y,_) in ghost.cells() {
                    sprites.add(draw_tile(
                        x+3, y, 2, 0, cell_color_to_color(piece.kind.0.color())
                    ));
                }
                for (x,y,_) in piece.cells() {
                    sprites.add(draw_tile(
                        x+3, y, 1, 0, cell_color_to_color(piece.kind.0.color())
                    ));
                }
            }
            State::LineClearAnimation(ref lines, frame) => {
                let frame_x = frame.min(35) / 12;
                let frame_y = frame.min(35) % 12;
                for &y in lines {
                    sprites.add(draw_tile(
                        3, y, frame_x*3+3, frame_y, WHITE
                    ));
                    sprites.add(draw_tile(
                        12, y, frame_x*3+5, frame_y, WHITE
                    ));
                    for x in 1..9 {
                        sprites.add(draw_tile(
                            x+3, y, frame_x*3+4, frame_y, WHITE
                        ));
                    }
                }
            }
            _ => {}
        }
        // Draw hold piece and next queue
        if let Some(piece) = self.hold_piece {
            draw_piece_preview(sprites, 0, 18, piece);
        }
        for (i, &piece) in self.next_queue.iter().enumerate() {
            draw_piece_preview(sprites, 13, 18 - (i*2) as i32, piece);
        }
        // Draw the pending garbage bar
        if self.garbage_queue > 0 {
            particle_mesh.rectangle(
                DrawMode::Fill(FillOptions::tolerance(0.1 / scale)),
                Rect {
                    x: 13.0,
                    w: 0.15,
                    y: 20.25 - self.garbage_queue as f32,
                    h: self.garbage_queue as f32
                },
                Color::from_rgb(255, 32, 32)
            );
        }
        queue_text(
            ctx, &text("Hold", scale, 3.0*scale), [text_x, scale*0.25], None
        );
        queue_text(
            ctx, &text("Next", scale, 3.0*scale), [text_x+13.0*scale, scale*0.25], None
        );
        queue_text(
            ctx, &text("Statistics", scale*0.75, 4.0*scale), [text_x-1.0*scale, scale*3.0], None
        );
        // Prepare statistics text
        let seconds = self.game_time as f32 / 60.0;
        let lines = vec![
            ("Pieces", format!("{}", self.statistics.pieces)),
            ("PPS", format!("{:.1}", self.statistics.pieces as f32 / seconds)),
            ("Lines", format!("{}", self.statistics.lines)),
            ("Attack", format!("{}", self.statistics.attack)),
            ("APM", format!("{:.1}", self.statistics.attack as f32 / seconds * 60.0)),
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
            ("Perfect", format!("{}", self.statistics.perfect_clears))
        ];
        // Draw statistics text
        let mut y = 3.75*scale;
        for (label, stat) in lines {
            queue_text(
                ctx, &text(label, scale*0.66, 0.0), [text_x-0.75*scale, y], None
            );
            queue_text(
                ctx, &text(stat, scale*0.66, -3.5*scale), [text_x-0.75*scale, y], None
            );
            y += scale * 0.66;
        }
        // Draw player name
        let y = (self.next_queue.len() as f32 * 2.0 + 1.0) * scale;
        queue_text(
            ctx, &text(&*self.name, scale*0.66, 3.5*scale), [text_x+13.25*scale, y], None
        );
        if let Some(ref info) = self.info {
            // Draw bot information
            let y = 12.0 * scale;
            queue_text(
                ctx, &text("Depth", scale*0.66, 0.0), [text_x-0.75*scale, y + 2.0*scale], None
            );
            queue_text(
                ctx,
                &text(format!("{}", info.depth), scale*0.66, -3.5*scale),
                [text_x-0.75*scale, y + 2.0*scale],
                None
            );
            queue_text(
                ctx, &text("Nodes", scale*0.66, 0.0), [text_x-0.75*scale, y + 2.7*scale], None
            );
            queue_text(
                ctx,
                &text(format!("{}", info.nodes), scale*0.66, -3.5*scale),
                [text_x-0.75*scale, y + 2.7*scale],
                None
            );
            queue_text(
                ctx, &text("Eval", scale*0.66, 0.0), [text_x-0.75*scale, y + 3.4*scale], None
            );
            queue_text(
                ctx,
                &text(format!("{}", info.evaluation), scale*0.66, -3.5*scale),
                [text_x-0.75*scale, y + 3.4*scale],
                None
            );
            // Draw plan description
            queue_text(
                ctx, &text("Plan:", scale*0.66, 0.0), [text_x-0.75*scale, y + 4.1*scale], None
            );
            let mut y = y + 4.1*scale;
            let mut x = text_x-0.75*scale;
            let mut has_pc = false;
            let mut has_send = false;
            for (_, lock) in &info.plan {
                x += 1.3 * scale;
                if x > text_x+2.25*scale {
                    x = text_x-0.75*scale;
                    y += 0.7 * scale;
                }
                queue_text(
                    ctx,
                    &text(
                        if lock.perfect_clear {
                            has_pc = true;
                            "PC"
                        } else {
                            if lock.placement_kind.is_hard() && lock.placement_kind.is_clear() {
                                has_send = true;
                            }
                            lock.placement_kind.short_name()
                        },
                        scale*0.66, 0.0
                    ),
                    [x, y],
                    None
                )
            }
            // Draw plan visualization
            if has_send || has_pc {
                let mut y_map = [0; 40];
                for i in 0..40 {
                    y_map[i] = i as i32;
                }
                for (placement, lock) in &info.plan {
                    for (x, y, d) in placement.location.cells() {
                        let (tx, ty) = dir_to_tile(d);
                        sprites.add(draw_tile(
                            x+3, y_map[y as usize],
                            tx, ty,
                            cell_color_to_color(placement.location.kind.0.color())
                        ));
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
                            || lock.perfect_clear {
                        break
                    }
                }
            }
        }
        // Draw clear info stuff
        if let Some(timer) = self.back_to_back_splash {
            queue_text(
                ctx,
                &text("Back-To-Back", scale, 0.0),
                [text_x+3.5*scale, 20.7*scale],
                Some(Color::new(1.0, 1.0, 1.0, timer.min(15) as f32 / 15.0))
            );
        }
        if let Some((combo, timer)) = self.combo_splash {
            queue_text(
                ctx,
                &text(format!("{} Combo", combo), scale, -9.0*scale),
                [text_x+3.5*scale, 20.7*scale],
                Some(Color::new(1.0, 1.0, 1.0, timer.min(15) as f32 / 15.0))
            );
        }
        if let Some((txt, timer)) = self.clear_splash {
            queue_text(
                ctx,
                &text(txt, scale, 9.0*scale),
                [text_x+3.5*scale, 21.6*scale],
                Some(Color::new(1.0, 1.0, 1.0, timer.min(15) as f32 / 15.0))
            );
        }
        Ok(())
    }
}

fn cell_color_to_color(cell_color: CellColor) -> Color {
    match cell_color {
        CellColor::Empty => WHITE,
        CellColor::Garbage => Color::from_rgb(160, 160, 160),
        CellColor::Unclearable => Color::from_rgb(64, 64, 64),
        CellColor::Z => Color::from_rgb(255, 32, 32),
        CellColor::S => Color::from_rgb(32, 255, 32),
        CellColor::O => Color::from_rgb(255, 255, 32),
        CellColor::L => Color::from_rgb(255, 143, 32),
        CellColor::J => Color::from_rgb(96, 96, 255),
        CellColor::I => Color::from_rgb(32, 255, 255),
        CellColor::T => Color::from_rgb(143, 32, 255)
    }
}

fn tile(x: i32, y: i32) -> Rect {
    Rect {
        x: x as f32 * (85.0/1024.0) + 1.0/1024.0,
        y: y as f32 * (85.0/1024.0) + 1.0/1024.0,
        h: 83.0/1024.0,
        w: 83.0/1024.0
    }
}

fn draw_piece_preview(sprites: &mut SpriteBatch, x: i32, y: i32, piece: Piece) {
    let ty = match piece {
        Piece::I => 1,
        Piece::O => 2,
        Piece::T => 3,
        Piece::L => 4,
        Piece::J => 5,
        Piece::S => 6,
        Piece::Z => 7
    };
    let color = cell_color_to_color(piece.color());
    for dx in 0..3 {
        if dx != 1 && piece == Piece::O { continue }
        sprites.add(draw_tile(x+dx, y, dx, ty, color));
    }
}

fn draw_tile(x: i32, y: i32, tx: i32, ty: i32, color: Color) -> DrawParam {
    DrawParam::new()
        .dest([x as f32, (20-y) as f32 - 0.75])
        .src(tile(tx, ty))
        .color(color)
        .scale([SPRITE_SCALE, SPRITE_SCALE])
}

const SPRITE_SCALE: f32 = 1.0/83.0;

fn dir_to_tile(dir: enumset::EnumSet<libtetris::Direction>) -> (i32, i32) {
    use libtetris::Direction::*;
    use enumset::EnumSet;
    if dir == EnumSet::only(Up) {
        (2, 10)
    } else if dir == EnumSet::only(Down) {
        (2, 8)
    } else if dir == EnumSet::only(Left) {
        (2, 11)
    } else if dir == EnumSet::only(Right) {
        (0, 11)
    } else if dir == Left | Right {
        (1, 11)
    } else if dir == Up | Down {
        (2, 9)
    } else if dir == Left | Up {
        (1, 10)
    } else if dir == Left | Down {
        (1, 9)
    } else if dir == Right | Up {
        (0, 10)
    } else if dir == Right | Down {
        (0, 9)
    } else if dir == Left | Right | Up {
        (0, 8)
    } else if dir == Left | Right | Down {
        (1, 8)
    } else if dir == Up | Down | Left {
        (2, 2)
    } else if dir == Up | Down | Right {
        (0, 2)
    } else {
        (2, 0)
    }
}