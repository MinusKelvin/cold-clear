use std::convert::Infallible;

use futures::{Sink, SinkExt, Stream, StreamExt};
use tbp::randomizer::RandomizerState;
use tbp::{BotMessage, FrontendMessage};

pub async fn run(
    mut incoming: impl Stream<Item = tbp::FrontendMessage> + Unpin,
    mut outgoing: impl Sink<tbp::BotMessage, Error = Infallible> + Unpin,
) {
    let mut bot = None;

    outgoing
        .send(BotMessage::Info {
            name: "Cold Clear".to_string(),
            version: "2020-05-05".to_string(),
            author: "MinusKelvin".to_string(),
            features: tbp::Feature::enabled(),
        })
        .await
        .unwrap();

    while let Some(msg) = incoming.next().await {
        match msg {
            FrontendMessage::Rules { randomizer: _ } => {
                outgoing.send(BotMessage::Ready).await.unwrap();
            }
            FrontendMessage::Start {
                hold,
                queue,
                combo,
                back_to_back,
                board,
                randomizer,
            } => {
                let mut b = libtetris::Board::new();
                b.hold_piece = hold.map(from_tbp_piece);
                for piece in queue {
                    b.add_next_piece(from_tbp_piece(piece));
                }
                if let RandomizerState::SevenBag { bag_state } = &randomizer {
                    b.bag = bag_state.iter().copied().map(from_tbp_piece).collect();
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

                let options = cold_clear::Options {
                    speculate: matches!(randomizer, RandomizerState::SevenBag { .. }),
                    ..Default::default()
                };
                let eval = cold_clear::evaluation::Standard::default();

                #[cfg(not(target_arch = "wasm32"))]
                {
                    bot = Some(cold_clear::Interface::launch(b, options, eval, None));
                }
                #[cfg(target_arch = "wasm32")]
                {
                    bot = Some(cold_clear::Interface::launch("worker.js", b, options, eval).await);
                }
            }
            FrontendMessage::Stop => {
                bot = None;
            }
            FrontendMessage::Suggest => {
                if let Some(ref mut bot) = bot {
                    bot.suggest_next_move(0);
                    #[cfg(not(target_arch = "wasm32"))]
                    let mvs = bot.block_next_move();
                    #[cfg(target_arch = "wasm32")]
                    let mvs = bot.block_next_move().await;
                    let moves =
                        mvs.map_or(vec![], |(mv, _)| vec![to_tbp_move(mv.expected_location)]);
                    outgoing
                        .send(BotMessage::Suggestion { moves })
                        .await
                        .unwrap();
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
            FrontendMessage::Quit => return,
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

#[cfg(target_arch = "wasm32")]
mod web {
    use futures::channel::mpsc::unbounded;
    use wasm_bindgen::{prelude::*, JsCast};
    use wasm_bindgen_futures::spawn_local;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = self)]
        static global: web_sys::DedicatedWorkerGlobalScope;
    }

    #[wasm_bindgen]
    pub fn start() {
        let (send, incoming) = unbounded();
        let closure = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            send.unbounded_send(e.data().into_serde().unwrap()).unwrap();
        }) as Box<dyn FnMut(web_sys::MessageEvent)>);

        global
            .add_event_listener_with_callback("message", closure.into_js_value().unchecked_ref())
            .unwrap();

        let outgoing = Box::pin(futures::sink::unfold((), |_, msg| {
            global
                .post_message(&JsValue::from_serde(&msg).unwrap())
                .unwrap();
            async { Ok(()) }
        }));

        spawn_local(crate::run(incoming, outgoing));
    }
}
