use ggez::{ Context, GameResult };
use ggez::graphics::*;
use ggez::graphics::spritebatch::SpriteBatch;
use std::collections::VecDeque;
use libtetris::*;
use arrayvec::ArrayVec;
use crate::interface::text;

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
    info_lines: Vec<(String, Option<String>)>
}

enum State {
    Falling(FallingPiece, FallingPiece),
    LineClearAnimation(ArrayVec<[i32; 4]>, i32),
    Delay
}

impl BoardDrawState {
    pub fn new(queue: impl IntoIterator<Item=Piece>) -> Self {
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
            info_lines: vec![]
        }
    }

    pub fn update(&mut self, update: GraphicsUpdate, time: u32) {
        self.garbage_queue = update.garbage_queue;
        self.game_time = time;
        if let Some(info) = update.info {
            self.info_lines = info;
        }
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
                    for (x, y) in piece.cells() {
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
        &self, ctx: &mut Context, sprites: &mut SpriteBatch, text_x: f32, scale: f32
    ) -> GameResult {
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
        match self.state {
            State::Falling(piece, ghost) => {
                for (x,y) in ghost.cells() {
                    sprites.add(draw_tile(
                        x+3, y, 2, 0, cell_color_to_color(piece.kind.0.color())
                    ));
                }
                for (x,y) in piece.cells() {
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
        if let Some(piece) = self.hold_piece {
            draw_piece_preview(ctx, sprites, 0, 18, piece);
        }
        for (i, &piece) in self.next_queue.iter().enumerate() {
            draw_piece_preview(ctx, sprites, 13, 18 - (i*2) as i32, piece);
        }
        if self.garbage_queue > 0 {
            let mesh = Mesh::new_rectangle(
                ctx,
                DrawMode::Fill(FillOptions::tolerance(0.1 / scale)),
                Rect {
                    x: 13.0,
                    w: 0.25,
                    y: 20.25 - self.garbage_queue as f32,
                    h: self.garbage_queue as f32
                },
                Color::from_rgb(255, 32, 32)
            )?;
            draw(ctx, &mesh, DrawParam::default())?;
        }
        queue_text(
            ctx, &text("Hold", scale, 3.0*scale), [text_x, scale*0.25], None
        );
        queue_text(
            ctx, &text("Next", scale, 3.0*scale), [text_x+13.0*scale, scale*0.25], None
        );
        queue_text(
            ctx, &text("Statistics", scale*0.75, 3.0*scale), [text_x, scale*3.0], None
        );
        let seconds = self.game_time as f32 / 60.0;
        let lines = vec![
            ("#", format!("{}", self.statistics.pieces)),
            ("PPS", format!("{:.1}", self.statistics.pieces as f32 / seconds)),
            ("Lines", format!("{}", self.statistics.lines)),
            ("ATK", format!("{}", self.statistics.attack)),
            ("APM", format!("{:.1}", self.statistics.attack as f32 / seconds * 60.0)),
            ("MxCb", format!("{}", self.statistics.max_combo)),
            ("S", format!("{}", self.statistics.singles)),
            ("D", format!("{}", self.statistics.doubles)),
            ("T", format!("{}", self.statistics.triples)),
            ("Tet", format!("{}", self.statistics.tetrises)),
            ("tsz", format!("{}", self.statistics.mini_tspin_zeros)),
            ("tss", format!("{}", self.statistics.mini_tspin_singles)),
            ("tsd", format!("{}", self.statistics.mini_tspin_doubles)),
            ("TSZ", format!("{}", self.statistics.tspin_zeros)),
            ("TSS", format!("{}", self.statistics.tspin_singles)),
            ("TSD", format!("{}", self.statistics.tspin_doubles)),
            ("TST", format!("{}", self.statistics.tspin_triples)),
            ("PC", format!("{}", self.statistics.perfect_clears))
        ];
        let mut y = 3.75*scale;
        for (label, stat) in lines {
            queue_text(
                ctx, &text(label, scale*0.66, 0.0), [text_x+0.25*scale, y], None
            );
            queue_text(
                ctx, &text(stat, scale*0.66, -2.5*scale), [text_x+0.25*scale, y], None
            );
            y += scale * 0.66;
        }
        let mut y = (self.next_queue.len() as f32 * 2.0 + 1.0) * scale;
        for (txt1, txt2) in &self.info_lines {
            match txt2 {
                None => queue_text(
                    ctx, &text(&**txt1, scale*0.66, 2.5*scale), [text_x+13.25*scale, y], None
                ),
                Some(txt2) => {
                    queue_text(
                        ctx, &text(&**txt1, scale*0.66, 0.0), [text_x+13.25*scale, y], None
                    );
                    queue_text(
                        ctx, &text(&**txt2, scale*0.66, -2.5*scale), [text_x+13.25*scale, y], None
                    )
                }
            }
            y += scale * 0.66
        }
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

fn draw_piece_preview(ctx: &mut Context, sprites: &mut SpriteBatch, x: i32, y: i32, piece: Piece) {
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
