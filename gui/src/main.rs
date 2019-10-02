use ggez::ContextBuilder;
use ggez::event;
use ggez::graphics::{ Image };
use ggez::graphics::spritebatch::SpriteBatch;
use ggez::audio;
use battle::GameConfig;

mod common;
mod local;
mod input;
mod interface;
mod replay;

use local::LocalGame;
use replay::ReplayGame;

pub struct Resources {
    sprites: SpriteBatch,

    move_sound: Option<audio::Source>,
    stack_touched: Option<audio::Source>,
    hard_drop: Option<audio::Source>,
    tspin: Option<audio::Source>,
    line_clear: Option<audio::Source>
}

fn main() {
    let mut replay = false;
    let mut replay_file = None;
    for arg in std::env::args() {
        if replay {
            replay_file = Some(arg);
            break
        }
        if arg == "--help" {
            println!("Cold Clear gameplay interface");
            println!("Options:");
            println!("  --play    <path>       View a replay");
            return
        } else if arg == "--play" {
            replay = true;
        }
    }
    if replay && replay_file.is_none() {
        eprintln!("--play requires argument");
        return
    }

    let mut cb = ContextBuilder::new("cold-clear", "MinusKelvin");
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut path = std::path::PathBuf::from(manifest_dir);
        path.push("resources");
        println!("Adding path {:?}", path);
        cb = cb.add_resource_path(path);
    }

    let (mut ctx, mut events) = cb
        .window_setup(ggez::conf::WindowSetup {
            title: "Cold Clear".to_owned(),
            ..Default::default()
        })
        .window_mode(ggez::conf::WindowMode {
            width: 1024.0,
            height: 576.0,
            resizable: true,
            ..Default::default()
        })
        .build().unwrap();

    let mut resources = Resources {
        sprites: SpriteBatch::new(Image::new(&mut ctx, "/sprites.png").unwrap()),
        move_sound: audio::Source::new(&mut ctx, "/move.ogg").or_else(|e| {
            eprintln!("Error loading sound effect for movement: {}", e);
            Err(e)
        }).ok(),
        stack_touched: audio::Source::new(&mut ctx, "/stack-touched.ogg").or_else(|e| {
            eprintln!("Error loading sound effect for stack touched: {}", e);
            Err(e)
        }).ok(),
        hard_drop: audio::Source::new(&mut ctx, "/hard-drop.ogg").or_else(|e| {
            eprintln!("Error loading sound effect for hard drop: {}", e);
            Err(e)
        }).ok(),
        tspin: audio::Source::new(&mut ctx, "/tspin.ogg").or_else(|e| {
            eprintln!("Error loading sound effect for T-spin: {}", e);
            Err(e)
        }).ok(),
        line_clear: audio::Source::new(&mut ctx, "/line-clear.ogg").or_else(|e| {
            eprintln!("Error loading sound effect for line clear: {}", e);
            Err(e)
        }).ok(),
    };

    match replay_file {
        Some(file) => {
            let mut replay_game = ReplayGame::new(&mut resources, file);
            event::run(&mut ctx, &mut events, &mut replay_game).unwrap();
        }
        None => {
            use bot::evaluation::*;
            use crate::input::*;
            let mut local_game = LocalGame::new(
                &mut resources,
                Box::new(|board| {
                    let evaluator = Standard {
                        move_time: -2,
                        b2b_clear: 100,
                        clear1: 0,
                        clear2: 100,
                        clear3: 200,
                        clear4: 400,
                        tspin1: 200,
                        tspin2: 400,
                        tspin3: 600,
                        mini_tspin1: 0,
                        mini_tspin2: 100,
                        perfect_clear: 1000,
                        sub_name: Some("New".to_owned()),
                        ..Standard::default()
                    };
                    let name = format!("Cold Clear\n{}", evaluator.name());
                    (Box::new(BotInput::new(bot::Interface::launch(
                        board,
                        bot::Options {
                            ..Default::default()
                        },
                        evaluator
                    ))), name)
                }),
                Box::new(|board|{
                    let evaluator = Standard {
                        sub_name: Some("Old".to_owned()),
                        ..Standard::default()
                    };
                    let name = format!("Cold Clear\n{}", evaluator.name());
                    (Box::new(BotInput::new(bot::Interface::launch(
                        board,
                        bot::Options {
                            ..Default::default()
                        },
                        evaluator
                    ))), name)
                }),
                GameConfig {
                    next_queue_size: 6,
                    ..Default::default()
                }
            );
            event::run(&mut ctx, &mut events, &mut local_game).unwrap();
        }
    }
}

