use std::io::{stdout, Result};

use tbp::{BotMessage, FrontendMessage};

fn main() -> Result<()> {
    let mut bot = None;

    send(&BotMessage::Info {
        name: "Cold Clear".to_string(),
        version: "2020-05-05".to_string(),
        author: "MinusKelvin".to_string(),
        features: tbp::Feature::enabled(),
    });

    loop {
        match receive() {
            FrontendMessage::Rules {} => {
                send(&BotMessage::Ready);
            }
            FrontendMessage::Start {
                hold,
                queue,
                combo,
                back_to_back,
                board,
            } => {
                let mut b = libtetris::Board::new();
                b.hold_piece = hold.map(from_tbp_piece);
                for piece in queue {
                    b.add_next_piece(from_tbp_piece(piece));
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
            FrontendMessage::Stop => {
                bot = None;
            }
            FrontendMessage::Suggest => {
                if let Some(ref mut bot) = bot {
                    bot.suggest_next_move(0);
                    let moves = bot
                        .block_next_move()
                        .map_or(vec![], |(mv, _)| vec![to_tbp_move(mv.expected_location)]);
                    send(&BotMessage::Suggestion { moves });
                }
            }
            FrontendMessage::Play { mv } => {
                if let Some(ref mut bot) = bot {
                    bot.play_next_move(from_tbp_move(mv));
                }
            }
            FrontendMessage::NewPiece { piece } => {
                if let Some(ref mut bot) = bot {
                    bot.add_next_piece(from_tbp_piece(piece));
                }
            }
            FrontendMessage::Quit => return Ok(()),
        }
    }
}

fn send(v: &BotMessage) {
    serde_json::to_writer(stdout(), v).unwrap();
    println!();
}

fn receive() -> FrontendMessage {
    let mut line = String::new();
    loop {
        line.clear();
        std::io::stdin().read_line(&mut line).unwrap();
        if let Ok(msg) = serde_json::from_str(&line) {
            return msg;
        }
    }
}

fn from_tbp_piece(v: tbp::Piece) -> libtetris::Piece {
    match v {
        tbp::Piece::I => libtetris::Piece::I,
        tbp::Piece::O => libtetris::Piece::O,
        tbp::Piece::T => libtetris::Piece::T,
        tbp::Piece::L => libtetris::Piece::L,
        tbp::Piece::J => libtetris::Piece::J,
        tbp::Piece::S => libtetris::Piece::S,
        tbp::Piece::Z => libtetris::Piece::Z,
    }
}

fn to_tbp_piece(v: libtetris::Piece) -> tbp::Piece {
    match v {
        libtetris::Piece::I => tbp::Piece::I,
        libtetris::Piece::O => tbp::Piece::O,
        libtetris::Piece::T => tbp::Piece::T,
        libtetris::Piece::L => tbp::Piece::L,
        libtetris::Piece::J => tbp::Piece::J,
        libtetris::Piece::S => tbp::Piece::S,
        libtetris::Piece::Z => tbp::Piece::Z,
    }
}

fn from_tbp_move(v: tbp::Move) -> libtetris::FallingPiece {
    libtetris::FallingPiece {
        kind: libtetris::PieceState(
            from_tbp_piece(v.location.kind),
            match v.location.orientation {
                tbp::Orientation::North => libtetris::RotationState::North,
                tbp::Orientation::South => libtetris::RotationState::South,
                tbp::Orientation::East => libtetris::RotationState::East,
                tbp::Orientation::West => libtetris::RotationState::West,
            },
        ),
        x: v.location.x,
        y: v.location.y,
        tspin: match v.spin {
            tbp::Spin::None => libtetris::TspinStatus::None,
            tbp::Spin::Mini => libtetris::TspinStatus::Mini,
            tbp::Spin::Full => libtetris::TspinStatus::Full,
        },
    }
}

fn to_tbp_move(v: libtetris::FallingPiece) -> tbp::Move {
    tbp::Move {
        location: tbp::PieceLocation {
            kind: to_tbp_piece(v.kind.0),
            orientation: match v.kind.1 {
                libtetris::RotationState::North => tbp::Orientation::North,
                libtetris::RotationState::South => tbp::Orientation::South,
                libtetris::RotationState::East => tbp::Orientation::East,
                libtetris::RotationState::West => tbp::Orientation::West,
            },
            x: v.x,
            y: v.y,
        },
        spin: match v.tspin {
            libtetris::TspinStatus::None => tbp::Spin::None,
            libtetris::TspinStatus::Mini => tbp::Spin::Mini,
            libtetris::TspinStatus::Full => tbp::Spin::Full,
        },
    }
}
