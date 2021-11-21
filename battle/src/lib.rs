use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

mod battle;
pub use battle::{Battle, BattleUpdate, PlayerUpdate, Replay};
mod controller;
pub use controller::PieceMoveExecutor;
mod game;
pub use game::{Event, Game};

/// Units are in ticks
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameConfig {
    pub spawn_delay: u32,
    pub line_clear_delay: u32,
    pub delayed_auto_shift: u32,
    pub auto_repeat_rate: u32,
    pub soft_drop_speed: u32,
    pub lock_delay: u32,
    /// Measured in 1/100 of a tick
    pub gravity: i32,

    pub next_queue_size: u32,
    pub max_garbage_add: u32,
    pub move_lock_rule: u32,
    pub garbage_blocking: bool,
    pub garbage_messiness: NotNan<f64>,
}

impl Default for GameConfig {
    fn default() -> Self {
        // Use something approximating Puyo Puyo Tetris
        GameConfig {
            spawn_delay: 7,
            line_clear_delay: 35,
            delayed_auto_shift: 9,
            auto_repeat_rate: 2,
            soft_drop_speed: 2,
            lock_delay: 30,
            gravity: 4500,
            next_queue_size: 5,
            max_garbage_add: 10,
            move_lock_rule: 15,
            garbage_blocking: false,
            garbage_messiness: NotNan::new(0.3).unwrap(),
        }
    }
}

impl GameConfig {
    pub fn fast_config() -> Self {
        GameConfig {
            spawn_delay: 0,
            line_clear_delay: 0,
            delayed_auto_shift: 8,
            auto_repeat_rate: 0,
            soft_drop_speed: 0,
            lock_delay: 30,
            gravity: 4500,
            next_queue_size: 5,
            max_garbage_add: 20,
            move_lock_rule: 15,
            garbage_blocking: true,
            garbage_messiness: NotNan::new(0.0).unwrap(),
        }
    }
}
