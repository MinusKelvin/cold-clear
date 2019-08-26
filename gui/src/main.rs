use ggez::{ Context, ContextBuilder, GameResult };
use ggez::event;
use std::net::SocketAddr;

mod common;
mod local;

use local::LocalGame;

fn main() {
    let mut connect = false;
    let mut address: Option<SocketAddr> = None;
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
        if arg == "--help" {
            println!("Cold Clear gameplay interface");
            println!("Options:");
            println!("  --connect <address>    Spectate an arena game");
            return
        } else if arg == "--connect" {
            connect = true;
        }
    }
    if connect && address.is_none() {
        eprintln!("--connect requires argument");
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
            // fullscreen_type: ggez::conf::FullscreenType::Desktop,
            ..Default::default()
        })
        .build().unwrap();

    match address {
        Some(addr) => {
            // let mut net_game = NetGame::new(&mut ctx, addr);
            // event::run(&mut ctx, &mut events, &mut net_game).unwrap();
        }
        None => {
            let mut local_game = LocalGame::new(&mut ctx);
            event::run(&mut ctx, &mut events, &mut local_game).unwrap();
        }
    }
}

