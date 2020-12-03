use game_util::{ TextRenderer, SpriteBatch, sprite_shader, Sound };
use game_util::rusttype::Font;
use game_util::rodio::{ self, Sink };

pub struct Resources {
    pub text: TextRenderer,
    pub sprites: sprites::Sprites,
    pub sprite_batch: SpriteBatch,
    pub line_clear: Sound,
    pub move_sound: Sound,
    pub hard_drop: Sound,
    pub move_sound_sink: Sink
}

impl Resources {
    pub fn load() -> Self {
        let mut text = TextRenderer::new();
        text.screen_size = (40.0, 23.0);
        text.add_style(vec![
            Font::try_from_bytes(include_bytes!("font/NotoSerif-Regular.ttf") as &[_]).unwrap()
        ]);

        let (sprites, sprite_sheet) = sprites::Sprites::load();
        let mut sprite_batch = SpriteBatch::new(sprite_shader(), sprite_sheet);
        sprite_batch.pixels_per_unit = 83.0;

        Resources {
            text, sprites, sprite_batch,
            line_clear: Sound::new(include_bytes!("sounds/line-clear.ogg") as &[_]),
            move_sound: Sound::new(include_bytes!("sounds/move.ogg") as &[_]),
            hard_drop: Sound::new(include_bytes!("sounds/hard-drop.ogg") as &[_]),
            move_sound_sink: Sink::new(&rodio::default_output_device().unwrap())
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/sprites.rs"));