use serde::{ Serialize, Deserialize };

#[macro_use]
extern crate rental;

pub mod evaluation;
pub mod moves;
mod modes;
mod dag;

#[cfg(not(target_arch = "wasm32"))]
mod desktop;
#[cfg(not(target_arch = "wasm32"))]
pub use desktop::Interface;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::Interface;

use libtetris::*;
pub use crate::moves::Move;
pub use crate::modes::normal::{ BotState, ThinkResult, Thinker };

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub mode: crate::moves::MovementMode,
    pub spawn_rule: SpawnRule,
    pub use_hold: bool,
    pub speculate: bool,
    pub pcloop: bool,
    pub min_nodes: u32,
    pub max_nodes: u32,
    pub threads: u32
}

#[derive(Serialize, Deserialize)]
enum BotMsg {
    Reset {
        #[serde(with = "BigArray")]
        field: [[bool; 10]; 40],
        b2b: bool,
        combo: u32
    },
    NewPiece(Piece),
    NextMove(u32),
    ForceAnalysisLine(Vec<FallingPiece>)
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Info {
    pub nodes: u32,
    pub depth: u32,
    pub original_rank: u32,
    pub evaluation_result: i32,
    pub plan: Vec<(FallingPiece, LockResult)>
}

#[derive(Serialize, Deserialize)]
pub enum BotPollState {
    Waiting,
    Dead
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: crate::moves::MovementMode::ZeroG,
            spawn_rule: SpawnRule::Row19Or20,
            use_hold: true,
            speculate: true,
            pcloop: false,
            min_nodes: 0,
            max_nodes: 4_000_000_000,
            threads: 1
        }
    }
}

use serde_big_array::big_array;
big_array!( BigArray; 40, );