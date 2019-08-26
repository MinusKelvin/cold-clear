use ggez::{ Context, GameResult };
use ggez::graphics::{ self, Color, DrawParam, Rect, Image };
use libtetris::*;
use arrayvec::ArrayVec;

pub struct BoardDrawState {
    board: ArrayVec<[ColoredRow; 40]>,
    state: State,

}

enum State {
    Falling(FallingPiece),
    LineClearAnimation(ArrayVec<[i32; 4]>),
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
                Event::PiecePlaced { piece, .. } => {
                    for (x, y) in piece.cells() {
                        self.board[y as usize].set(x as usize, piece.kind.0.color());
                    }
                }
                Event::PieceFalling(piece) => {
                    self.state = State::Falling(*piece);
                }
                _ => {}
            }
        }
    }

    pub fn draw(&self, ctx: &mut Context, img: &Image) -> GameResult {
        for y in 0..21 {
            for x in 0..10 {
                graphics::draw(ctx, img, DrawParam::new()
                    .dest([x as f32, (20-y) as f32])
                    .src(if self.board[y].cell_color(x) == CellColor::Empty
                        { tile(0, 0) } else { tile(1, 0) })
                    .color(cell_color_to_color(self.board[y].cell_color(x)))
                    .scale([1.0/30.0, 1.0/30.0]))?;
            }
        }
        match self.state {
            State::Falling(piece) => {
                for (x,y) in piece.cells() {
                    graphics::draw(ctx, img, DrawParam::new()
                        .dest([x as f32, (20-y) as f32])
                        .src(tile(1, 0))
                        .color(cell_color_to_color(piece.kind.0.color()))
                        .scale([1.0/30.0, 1.0/30.0]))?;
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
        CellColor::Z => Color::from_rgb(255, 0, 0),
        CellColor::S => Color::from_rgb(0, 255, 0),
        CellColor::O => Color::from_rgb(255, 255, 0),
        CellColor::L => Color::from_rgb(255, 127, 0),
        CellColor::J => Color::from_rgb(0, 0, 255),
        CellColor::I => Color::from_rgb(0, 255, 255),
        CellColor::T => Color::from_rgb(127, 0, 255),
        _ => graphics::WHITE
    }
}

fn tile(x: i32, y: i32) -> Rect {
    Rect {
        x: x as f32 / 16.0 + 1.0/512.0,
        y: y as f32 / 16.0 + 1.0/512.0,
        h: 30.0/512.0,
        w: 30.0/512.0
    }
}