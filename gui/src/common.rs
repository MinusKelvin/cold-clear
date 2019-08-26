use ggez::{ Context, GameResult };
use ggez::graphics::{ self, Color, DrawParam, Rect, Image };
use libtetris::*;
use arrayvec::ArrayVec;

pub struct BoardDrawState {
    board: ArrayVec<[ColoredRow; 40]>,
    state: State,

}

enum State {
    Falling(FallingPiece, FallingPiece),
    LineClearAnimation(ArrayVec<[i32; 4]>, i32),
    Delay
}

impl BoardDrawState {
    pub fn new() -> Self {
        BoardDrawState {
            board: ArrayVec::from([*ColoredRow::EMPTY; 40]),
            state: State::Delay
        }
    }

    pub fn update(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::PiecePlaced { piece, locked, .. } => {
                    for (x, y) in piece.cells() {
                        self.board[y as usize].set(x as usize, piece.kind.0.color());
                    }
                    if locked.cleared_lines.is_empty() {
                        self.state = State::Delay;
                    } else {
                        self.state = State::LineClearAnimation(locked.cleared_lines.clone(), 0);
                    }
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
                _ => {}
            }
        }
    }

    pub fn draw(&self, ctx: &mut Context, img: &Image) -> GameResult {
        for y in 0..21 {
            for x in 0..10 {
                graphics::draw(ctx, img, DrawParam::new()
                    .dest([x as f32, (20-y) as f32 - 0.75])
                    .src(if self.board[y].cell_color(x) == CellColor::Empty
                        { tile(0, 0) } else { tile(1, 0) })
                    .color(cell_color_to_color(self.board[y].cell_color(x)))
                    .scale([SPRITE_SCALE, SPRITE_SCALE]))?;
            }
        }
        match self.state {
            State::Falling(piece, ghost) => {
                for (x,y) in ghost.cells() {
                    graphics::draw(ctx, img, DrawParam::new()
                        .dest([x as f32, (20-y) as f32 - 0.75])
                        .src(tile(0, 1))
                        .color(cell_color_to_color(piece.kind.0.color()))
                        .scale([SPRITE_SCALE, SPRITE_SCALE]))?;
                }
                for (x,y) in piece.cells() {
                    graphics::draw(ctx, img, DrawParam::new()
                        .dest([x as f32, (20-y) as f32 - 0.75])
                        .src(tile(1, 0))
                        .color(cell_color_to_color(piece.kind.0.color()))
                        .scale([SPRITE_SCALE, SPRITE_SCALE]))?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn cell_color_to_color(cell_color: CellColor) -> Color {
    match cell_color {
        CellColor::Empty => graphics::WHITE,
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

const SPRITE_SCALE: f32 = 1.0/83.0;
