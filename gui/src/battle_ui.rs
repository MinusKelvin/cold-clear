use game_util::{prelude::*, text::Alignment};
use battle::{ Battle, BattleUpdate };
use crate::player_draw::PlayerDrawState;
use crate::res::Resources;

pub struct BattleUi {
    player_1_graphics: PlayerDrawState,
    player_2_graphics: PlayerDrawState,
    time: u32
}

impl BattleUi {
    pub fn new(battle: &Battle, p1_name: String, p2_name: String) -> Self {
        BattleUi {
            player_1_graphics: PlayerDrawState::new(battle.player_1.board.next_queue(), p1_name),
            player_2_graphics: PlayerDrawState::new(battle.player_2.board.next_queue(), p2_name),
            time: 0
        }
    }

    pub fn update(
        &mut self,
        res: &mut Resources,
        update: BattleUpdate,
        p1_info_update: Option<cold_clear::Info>,
        p2_info_update: Option<cold_clear::Info>
    ) {
        let mut move_sound_played_this_frame = false;
        for event in update.player_1.events.iter().chain(update.player_2.events.iter()) {
            use battle::Event::*;
            match event {
                PieceMoved | SoftDropped | PieceRotated => if !move_sound_played_this_frame {
                    res.sound_service.play(&res.move_sound);
                    move_sound_played_this_frame = true;
                }
                PiecePlaced { hard_drop_distance, locked, .. } => {
                    if hard_drop_distance.is_some() {
                        res.sound_service.play(&res.hard_drop);
                    }
                    if locked.placement_kind.is_clear() {
                        res.sound_service.play(&res.line_clear);
                    }
                }
                _ => {}
            }
        }

        self.player_1_graphics.update(update.player_1, p1_info_update, update.time);
        self.player_2_graphics.update(update.player_2, p2_info_update, update.time);
        self.time = update.time;
    }

    pub fn draw(&self, res: &mut Resources) {
        res.text.draw_text(
            &format!("{}:{:02}", self.time / 60 / 60, self.time / 60 % 60),
            20.0, 1.5,
            Alignment::Center,
            [0xFF; 4], 1.0, 0
        );

        self.player_1_graphics.draw(res, 0.0+1.0);
        self.player_2_graphics.draw(res, 20.0+1.0);

        res.sprite_batch.render(Transform3D::ortho(
            0.0, 40.0,
            0.0, 23.0,
            -1.0, 1.0
        ));
    }
}