use std::sync::mpsc::{Sender, channel};

use game_util::prelude::*;
use game_util::{ TextRenderer, SpriteBatch, sprite_shader, Sound };
use game_util::rusttype::Font;
use game_util::rodio::{ self, Sink };

pub struct Resources {
    pub text: TextRenderer,
    pub sprites: sprites::Sprites,
    pub sprite_batch: SpriteBatch,
    pub sound_player: Sender<SoundId>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SoundId {
    LineClear,
    MoveSound,
    HardDrop,
}

impl Resources {
    pub fn load(gl: &Gl) -> Self {
        let mut text = TextRenderer::new(gl).unwrap();
        text.screen_size = (40.0, 23.0);
        text.add_style(vec![
            Font::try_from_bytes(include_bytes!("font/NotoSerif-Regular.ttf") as &[_]).unwrap()
        ]);

        let (sprites, sprite_sheet) = sprites::Sprites::load(gl).unwrap();
        let mut sprite_batch = SpriteBatch::new(gl, sprite_shader(gl), sprite_sheet).unwrap();
        sprite_batch.pixels_per_unit = 83.0;

        let (sound_player, recv) = channel();
        std::thread::spawn(move || {
            let line_clear = Sound::new(include_bytes!("sounds/line-clear.ogg") as &[_]);
            let move_sound = Sound::new(include_bytes!("sounds/move.ogg") as &[_]);
            let hard_drop = Sound::new(include_bytes!("sounds/hard-drop.ogg") as &[_]);
            let move_sound_sink = Sink::new(&rodio::default_output_device().unwrap());
            while let Ok(sound) = recv.recv() {
                match sound {
                    SoundId::LineClear => line_clear.play(),
                    SoundId::HardDrop => hard_drop.play(),
                    SoundId::MoveSound => {
                        if move_sound_sink.len() <= 1 {
                            move_sound_sink.append(move_sound.sound());
                        }
                    }
                }
            }
        });

        Resources {
            text, sprites, sprite_batch, sound_player
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/sprites.rs"));