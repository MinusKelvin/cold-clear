use game_util::prelude::*;
use battle::{ Battle, BattleUpdate };
use crate::player_draw::PlayerDrawState;
use crate::res::Resources;

pub struct BattleUi {
    player_1_graphics: PlayerDrawState,
    player_2_graphics: PlayerDrawState,
    time: u32,
    multiplier: f32,
    move_sound_play: u32
}

impl BattleUi {
    pub fn new(battle: &Battle, p1_name: String, p2_name: String) -> Self {
        BattleUi {
            player_1_graphics: PlayerDrawState::new(battle.player_1.board.next_queue(), p1_name),
            player_2_graphics: PlayerDrawState::new(battle.player_2.board.next_queue(), p2_name),
            time: 0,
            multiplier: 1.0,
            move_sound_play: 0
        }
    }

    pub fn update(
        &mut self,
        update: BattleUpdate,
        p1_info_update: Option<cold_clear::Info>,
        p2_info_update: Option<cold_clear::Info>
    ) {
        for event in update.player_1.events.iter().chain(update.player_2.events.iter()) {
            use battle::Event::*;
            match event {
                // TODO: sound playback
                // PieceMoved | SoftDropped | PieceRotated => if self.move_sound_play == 0 {
                //     if let Some(move_sound) = &mut res.move_sound {
                //         move_sound.play_detached()?;
                //     }
                //     self.move_sound_play = 2;
                // }
                // PiecePlaced { hard_drop_distance, locked, .. } => {
                //     if hard_drop_distance.is_some() {
                //         if let Some(hard_drop) = &mut res.hard_drop {
                //             hard_drop.play_detached()?;
                //         }
                //     }
                //     if locked.placement_kind.is_clear() {
                //         if let Some(line_clear) = &mut res.line_clear {
                //             line_clear.play_detached()?;
                //         }
                //     }
                // }
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
    }

    pub fn draw(&self, res: &mut Resources) {
        self.player_1_graphics.draw(res, -17.5);
        self.player_2_graphics.draw(res, 0.0);

        res.sprite_batch.render(Transform3D::ortho(
            -17.5, 17.5,
            0.0, 23.0,
            -1.0, 1.0
        ));
    }
}