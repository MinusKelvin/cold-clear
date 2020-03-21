use game_util::{ TextRenderer, SpriteBatch, sprite_shader };
use game_util::rusttype::Font;

pub struct Resources {
    pub text: TextRenderer,
    pub sprites: sprites::Sprites,
    pub sprite_batch: SpriteBatch
}

impl Resources {
    pub fn load() -> Self {
        let mut text = TextRenderer::new();
        text.screen_size = (35.0, 23.0);
        text.add_style(vec![
            Font::from_bytes(include_bytes!("font/NotoSerif-Regular.ttf") as &[_]).unwrap()
        ]);

        let (sprites, sprite_sheet) = sprites::Sprites::load();
        let mut sprite_batch = SpriteBatch::new(sprite_shader(), sprite_sheet);
        sprite_batch.pixels_per_unit = 83.0;

        Resources {
            text, sprites, sprite_batch
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/sprites.rs"));