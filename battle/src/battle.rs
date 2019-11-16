use rand_pcg::Pcg64Mcg;
use rand::prelude::*;
use std::collections::VecDeque;
use serde::{ Serialize, Deserialize };
use crate::{ Game, GameConfig, Controller, Event };

pub struct Battle {
    pub player_1: Game,
    pub player_2: Game,
    p1_rng: Pcg64Mcg,
    p2_rng: Pcg64Mcg,
    garbage_rng: Pcg64Mcg,
    pub time: u32,
    multiplier: f32,
    margin_time: Option<u32>,
    pub replay: Replay
}

impl Battle {
    pub fn new(
        p1_config: GameConfig, p2_config: GameConfig,
        p1_seed: <Pcg64Mcg as SeedableRng>::Seed,
        p2_seed: <Pcg64Mcg as SeedableRng>::Seed,
        garbage_seed: <Pcg64Mcg as SeedableRng>::Seed
    ) -> Self {
        let mut p1_rng = Pcg64Mcg::from_seed(p1_seed);
        let mut p2_rng = Pcg64Mcg::from_seed(p2_seed);
        let garbage_rng = Pcg64Mcg::from_seed(garbage_seed);
        let player_1 = Game::new(p1_config, &mut p1_rng);
        let player_2 = Game::new(p2_config, &mut p2_rng);
        Battle {
            replay: Replay {
                p1_name: String::new(), p2_name: String::new(),
                p1_config, p2_config, p1_seed, p2_seed, garbage_seed,
                updates: VecDeque::new()
            },
            player_1, player_2,
            p1_rng, p2_rng, garbage_rng,
            time: 0,
            margin_time: p1_config.margin_time,
            multiplier: 1.0,
        }
    }

    pub fn update(&mut self, p1: Controller, p2: Controller) -> BattleUpdate {
        self.time += 1;
        if let Some(margin_time) = self.margin_time {
            if self.time >= margin_time && (self.time - margin_time) % 1800 == 0 {
                self.multiplier += 0.5;
            }
        }

        self.replay.updates.push_back((p1, p2));

        let p1_events = self.player_1.update(p1, &mut self.p1_rng, &mut self.garbage_rng);
        let p2_events = self.player_2.update(p2, &mut self.p2_rng, &mut self.garbage_rng);

        for event in &p1_events {
            if let &Event::GarbageSent(amt) = event {
                self.player_2.garbage_queue += (amt as f32 * self.multiplier) as u32;
            }
        }
        for event in &p2_events {
            if let &Event::GarbageSent(amt) = event {
                self.player_1.garbage_queue += (amt as f32 * self.multiplier) as u32;
            }
        }

        BattleUpdate {
            player_1: PlayerUpdate {
                events: p1_events,
                garbage_queue: self.player_1.garbage_queue
            },
            player_2: PlayerUpdate {
                events: p2_events,
                garbage_queue: self.player_2.garbage_queue
            },
            time: self.time,
            attack_multiplier: self.multiplier
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BattleUpdate {
    pub player_1: PlayerUpdate,
    pub player_2: PlayerUpdate,
    pub time: u32,
    pub attack_multiplier: f32
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerUpdate {
    pub events: Vec<Event>,
    pub garbage_queue: u32
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Replay {
    pub p1_name: String,
    pub p2_name: String,
    pub p1_seed: <Pcg64Mcg as SeedableRng>::Seed,
    pub p2_seed: <Pcg64Mcg as SeedableRng>::Seed,
    pub garbage_seed: <Pcg64Mcg as SeedableRng>::Seed,
    pub p1_config: GameConfig,
    pub p2_config: GameConfig,
    pub updates: VecDeque<(Controller, Controller)>
}