use game_util::prelude::*;
use game_util::rusttype::Font;
use game_util::sound::{Sound, SoundService};
use game_util::sprite::{sprite_shader, SpriteBatch};
use game_util::text::TextRenderer;
use game_util::LocalExecutor;

pub struct Resources {
    pub text: TextRenderer,
    pub sprites: sprites::Sprites,
    pub sprite_batch: SpriteBatch,
    pub sound_service: SoundService,
    pub line_clear: Sound,
    pub move_sound: Sound,
    pub hard_drop: Sound,
}

impl Resources {
    pub async fn load(gl: &Gl, executor: &LocalExecutor) -> Self {
        let (noto, (sprites, sprite_sheet), line_clear, move_sound, hard_drop) = game_util::futures::try_join!(
            async {
                Font::try_from_vec(
                    game_util::load_binary("res/NotoSerif-Regular.ttf")
                        .await
                        .unwrap(),
                ).ok_or("Failed to load Noto Serif font".to_owned())
            },
            sprites::Sprites::load(gl, "res/generated"),
            Sound::load("res/line-clear.ogg"),
            Sound::load("res/move.ogg"),
            Sound::load("res/hard-drop.ogg")
        ).unwrap();

        let mut text = TextRenderer::new(gl).unwrap();
        text.screen_size = (40.0, 23.0);
        text.add_style(Some(noto));

        let mut sprite_batch = SpriteBatch::new(gl, sprite_shader(gl), sprite_sheet).unwrap();
        sprite_batch.pixels_per_unit = 83.0;

        Resources {
            text,
            sprites,
            sprite_batch,
            move_sound,
            hard_drop,
            line_clear,
            sound_service: SoundService::new(executor),
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/sprites.rs"));
