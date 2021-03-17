use std::io::{stdout, Result};

use tbi::Message;

mod tbi;

fn main() -> Result<()> {
    let mut bot = None;

    serde_json::to_writer(
        stdout(),
        &Message::Ready {
            name: "Cold Clear".to_string(),
            version: "2020-03-17".to_string(),
            author: "MinusKelvin".to_string(),
        },
    )?;
    println!();

    let mut line = String::new();
    loop {
        line.clear();
        std::io::stdin().read_line(&mut line)?;
        let msg = serde_json::from_str(&line)?;
        match msg {
            Message::Start {
                hold,
                queue,
                combo,
                back_to_back,
                board,
            } => {
                let mut b = libtetris::Board::new();
                b.hold_piece = hold.map(Into::into);
                for piece in queue {
                    b.add_next_piece(piece.into());
                }
                b.combo = combo;
                b.b2b_bonus = back_to_back;
                let mut field = [[false; 10]; 40];
                for y in 0..40 {
                    for x in 0..10 {
                        field[y][x] = board[y][x].is_some();
                    }
                }
                b.set_field(field);

                bot = Some(cold_clear::Interface::launch(
                    b,
                    cold_clear::Options {
                        speculate: false,
                        ..Default::default()
                    },
                    cold_clear::evaluation::Standard::default(),
                    None,
                ));
            }
            Message::Stop => {
                bot = None;
            }
            Message::Suggest => {
                if let Some(ref mut bot) = bot {
                    bot.suggest_next_move(0);
                    let moves = bot
                        .block_next_move()
                        .map_or(vec![], |(mv, _)| vec![mv.expected_location.into()]);
                    serde_json::to_writer(stdout(), &Message::Suggestion { moves })?;
                    println!();
                }
            }
            Message::Play { mv } => {
                if let Some(ref mut bot) = bot {
                    bot.play_next_move(mv.into());
                }
            }
            Message::NewPiece { piece } => {
                if let Some(ref mut bot) = bot {
                    bot.add_next_piece(piece.into());
                }
            }
            Message::Quit => return Ok(()),
            _ => {}
        }
    }
}

impl From<tbi::Piece> for libtetris::Piece {
    fn from(v: tbi::Piece) -> libtetris::Piece {
        match v {
            tbi::Piece::I => libtetris::Piece::I,
            tbi::Piece::O => libtetris::Piece::O,
            tbi::Piece::T => libtetris::Piece::T,
            tbi::Piece::L => libtetris::Piece::L,
            tbi::Piece::J => libtetris::Piece::J,
            tbi::Piece::S => libtetris::Piece::S,
            tbi::Piece::Z => libtetris::Piece::Z,
        }
    }
}

impl From<libtetris::Piece> for tbi::Piece {
    fn from(v: libtetris::Piece) -> tbi::Piece {
        match v {
            libtetris::Piece::I => tbi::Piece::I,
            libtetris::Piece::O => tbi::Piece::O,
            libtetris::Piece::T => tbi::Piece::T,
            libtetris::Piece::L => tbi::Piece::L,
            libtetris::Piece::J => tbi::Piece::J,
            libtetris::Piece::S => tbi::Piece::S,
            libtetris::Piece::Z => tbi::Piece::Z,
        }
    }
}

impl From<tbi::Move> for libtetris::FallingPiece {
    fn from(v: tbi::Move) -> libtetris::FallingPiece {
        libtetris::FallingPiece {
            kind: libtetris::PieceState(
                v.location.kind.into(),
                match v.location.orientation {
                    tbi::Orientation::North => libtetris::RotationState::North,
                    tbi::Orientation::South => libtetris::RotationState::South,
                    tbi::Orientation::East => libtetris::RotationState::East,
                    tbi::Orientation::West => libtetris::RotationState::West,
                },
            ),
            x: v.location.x,
            y: v.location.y,
            tspin: match v.spin {
                tbi::Spin::None => libtetris::TspinStatus::None,
                tbi::Spin::Mini => libtetris::TspinStatus::Mini,
                tbi::Spin::Full => libtetris::TspinStatus::Full,
            }
        }
    }
}

impl From<libtetris::FallingPiece> for tbi::Move {
    fn from(v: libtetris::FallingPiece) -> tbi::Move {
        tbi::Move {
            location: tbi::PieceLocation {
                kind: v.kind.0.into(),
                orientation: match v.kind.1 {
                    libtetris::RotationState::North => tbi::Orientation::North,
                    libtetris::RotationState::South => tbi::Orientation::South,
                    libtetris::RotationState::East => tbi::Orientation::East,
                    libtetris::RotationState::West => tbi::Orientation::West,
                },
                x: v.x,
                y: v.y,
            },
            spin: match v.tspin {
                libtetris::TspinStatus::None => tbi::Spin::None,
                libtetris::TspinStatus::Mini => tbi::Spin::Mini,
                libtetris::TspinStatus::Full => tbi::Spin::Full,
            }
        }
    }
}