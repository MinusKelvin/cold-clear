use ggez::{ Context, ContextBuilder, GameResult };
use ggez::event;
use ggez::graphics::{ Image };
use ggez::graphics::spritebatch::SpriteBatch;
use ggez::audio::{ self, SoundSource };
use std::net::SocketAddr;

mod common;
mod local;
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
    let mut connect = false;
    let mut replay = false;
    let mut address: Option<SocketAddr> = None;
    let mut replay_file = None;
    for arg in std::env::args() {
        if connect {
            match arg.parse() {
                Ok(addr) => address = Some(addr),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    return
                }
            }
            break
        }
        if replay {
            replay_file = Some(arg);
            break
        }
        if arg == "--help" {
            println!("Cold Clear gameplay interface");
            println!("Options:");
            println!("  --connect <address>    Spectate an arena");
            println!("  --play    <path>       View a replay");
            return
        } else if arg == "--connect" {
            connect = true;
        } else if arg == "--play" {
            replay = true;
        }
    }
    if connect && address.is_none() {
        eprintln!("--connect requires argument");
        return
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

    match (address, replay_file) {
        (Some(addr), _) => {
            // let mut net_game = NetGame::new(&mut ctx, addr);
            // event::run(&mut ctx, &mut events, &mut net_game).unwrap();
        }
        (None, Some(file)) => {
            let mut replay_game = ReplayGame::new(&mut resources, file);
            event::run(&mut ctx, &mut events, &mut replay_game).unwrap();
        }
        (None, None) => {
            let mut local_game = LocalGame::new(&mut resources);
            event::run(&mut ctx, &mut events, &mut local_game).unwrap();
        }
    }
}

