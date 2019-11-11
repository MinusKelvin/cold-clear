#![windows_subsystem = "windows"]

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
use crate::input::Keyboard;

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
        stack_touched: None,
        // stack_touched: audio::Source::new(&mut ctx, "/stack-touched.ogg").or_else(|e| {
        //     eprintln!("Error loading sound effect for stack touched: {}", e);
        //     Err(e)
        // }).ok(),
        hard_drop: audio::Source::new(&mut ctx, "/hard-drop.ogg").or_else(|e| {
            eprintln!("Error loading sound effect for hard drop: {}", e);
            Err(e)
        }).ok(),
        tspin: None,
        // tspin: audio::Source::new(&mut ctx, "/tspin.ogg").or_else(|e| {
        //     eprintln!("Error loading sound effect for T-spin: {}", e);
        //     Err(e)
        // }).ok(),
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
            use bot::evaluation::{ self, Evaluator };
            use crate::input::BotInput;
            let input = match read_controls() {
                Ok(controls) => controls,
                Err(e) => {
                    eprintln!("An error occured while loading the controls: {}", e);
                    Keyboard::default()
                }
            };
            let mut local_game = LocalGame::new(
                &mut resources,
                Box::new(move |board| {
                    let evaluator = evaluation::changed::Standard {
                        ..Default::default()
                    };
                    let name = format!("Cold Clear\n{}", evaluator.name());
                    (Box::new(BotInput::new(bot::Interface::launch(
                        board,
                        bot::Options {
                            ..Default::default()
                        },
                        evaluator
                    ))), name)
                    // (Box::new(input), "Human".to_owned())
                }),
                Box::new(|board|{
                    let evaluator = evaluation::Standard {
                        ..Default::default()
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

fn read_controls() -> Result<Keyboard, Box<dyn std::error::Error>> {
    match std::fs::read_to_string("controls.toml") {
        Ok(controls) => Ok(toml::from_str(&controls)?),
        Err(e) => if e.kind() == std::io::ErrorKind::NotFound {
            let ser = toml::to_string_pretty(&Keyboard::default())?;
            let mut s = include_str!("control-help").to_owned();
            s.push_str(&ser);
            std::fs::write("controls.toml", &s)?;
            Ok(Keyboard::default())
        } else {
            Err(e.into())
        }
    }
}
