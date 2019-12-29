use crate::common::BoardDrawState;
use crate::Resources;
use ggez::{ Context, GameResult };
use ggez::audio::SoundSource;
use ggez::graphics::*;
use battle::{ BattleUpdate, Battle };

pub struct Gui {
    player_1_graphics: BoardDrawState,
    player_2_graphics: BoardDrawState,
    time: u32,
    multiplier: f32,
    move_sound_play: u32
}

impl Gui {
    pub fn new(battle: &Battle, p1_name: String, p2_name: String) -> Self {
        Gui {
            player_1_graphics: BoardDrawState::new(battle.player_1.board.next_queue(), p1_name),
            player_2_graphics: BoardDrawState::new(battle.player_2.board.next_queue(), p2_name),
            time: 0,
            multiplier: 1.0,
            move_sound_play: 0
        }
    }

    pub fn update(
        &mut self,
        update: BattleUpdate,
        p1_info_update: Option<cold_clear::Info>,
        p2_info_update: Option<cold_clear::Info>,
        res: &mut Resources
    ) -> GameResult {
        for event in update.player_1.events.iter().chain(update.player_2.events.iter()) {
            use battle::Event::*;
            match event {
                PieceMoved | SoftDropped | PieceRotated => if self.move_sound_play == 0 {
                    if let Some(move_sound) = &mut res.move_sound {
                        move_sound.play_detached()?;
                    }
                    self.move_sound_play = 2;
                }
                // StackTouched => self.stack_touched.play_detached()?,
                // PieceTSpined => self.tspin.play_detached()?,
                PiecePlaced { hard_drop_distance, locked, .. } => {
                    if hard_drop_distance.is_some() {
                        if let Some(hard_drop) = &mut res.hard_drop {
                            hard_drop.play_detached()?;
                        }
                    }
                    if locked.placement_kind.is_clear() {
                        if let Some(line_clear) = &mut res.line_clear {
                            line_clear.play_detached()?;
                        }
                    }
                }
                _ => {}
            }
        }
        if self.move_sound_play != 0 {
            self.move_sound_play -= 1;
        }

        self.player_1_graphics.update(update.player_1, p1_info_update, update.time);
        self.player_2_graphics.update(update.player_2, p2_info_update, update.time);
        self.time = update.time;
        self.multiplier = update.attack_multiplier;

        Ok(())
    }

    pub fn draw(
        &mut self, ctx: &mut Context, res: &mut Resources, scale: f32, center: f32
    ) -> GameResult<()> {
        push_transform(ctx, Some(DrawParam::new()
            .scale([scale, scale])
            .dest([center - 17.5 * scale, 0.0])
            .to_matrix()));
        apply_transformations(ctx)?;

        res.sprites.clear();
        let mut mesh = MeshBuilder::new();
        self.player_1_graphics.draw(ctx, &mut res.sprites, &mut mesh, center - 17.5*scale, scale)?;
        draw(ctx, &res.sprites, DrawParam::default())?;
        if let Ok(mesh) = mesh.build(ctx) {
            draw(ctx, &mesh, DrawParam::default())?;
        }

        pop_transform(ctx);

        push_transform(ctx, Some(DrawParam::new()
            .scale([scale, scale])
            .dest([center + 1.5*scale, 0.0])
            .to_matrix()));
        apply_transformations(ctx)?;

        let mut mesh = MeshBuilder::new();
        res.sprites.clear();
        self.player_2_graphics.draw(ctx, &mut res.sprites, &mut mesh, center+1.5*scale, scale)?;
        draw(ctx, &res.sprites, DrawParam::default())?;
        if let Ok(mesh) = mesh.build(ctx) {
            draw(ctx, &mesh, DrawParam::default())?;
        }

        pop_transform(ctx);

        queue_text(
            ctx,
            &text(
                format!("{}:{:02}", self.time / 60 / 60, self.time / 60 % 60),
                scale*1.5, 8.0*scale
            ),
            [center-4.0*scale, 20.6*scale],
            None
        );
        if self.multiplier != 1.0 {
            queue_text(
                ctx,
                &text(format!("Margin Time: x{:.1}", self.multiplier), scale*1.0, 8.0*scale),
                [center-4.0*scale, 21.9*scale],
                None
            );
        }

        apply_transformations(ctx)?;
        draw_queued_text(
            ctx, DrawParam::new(), None, FilterMode::Linear
        )?;

        Ok(())
    }
}

/// Returns (scale, center)
pub fn setup_graphics(ctx: &mut Context) -> GameResult<(f32, f32)> {
    clear(ctx, BLACK);
    let dpi = window(ctx).get_hidpi_factor() as f32;
    let size = drawable_size(ctx);
    let size = (size.0 * dpi, size.1 * dpi);
    let center = size.0 / 2.0;
    let scale = size.1 / 23.0;
    set_screen_coordinates(ctx, Rect {
        x: 0.0, y: 0.0, w: size.0, h: size.1
    })?;

    Ok((scale, center))
}

pub fn text(s: impl Into<TextFragment>, ts: f32, width: f32) -> Text {
    let mut text = Text::new(s);
    text.set_font(Default::default(), Scale::uniform(ts*0.75));
    if width != 0.0 {
        if width < 0.0 {
            text.set_bounds([-width, 1230.0], Align::Right);
        } else {
            text.set_bounds([width, 1230.0], Align::Center);
        }
    }
    text
}