use libtetris::{ LockResult, Board, Piece };
use serde::{ Serialize, Deserialize };

mod misalike;
pub use self::misalike::Misalike;
mod standard;
pub use self::standard::Standard;
pub mod changed;

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchOptions {
    pub aggressive_height: i32,
    pub defensive_height: i32,
    pub gamma: (i32, i32)
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Evaluation {
    pub aggressive_accumulated: i32,
    pub aggressive_transient: i32,
    pub defensive_accumulated: i32,
    pub defensive_transient: i32
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Eval {
    pub aggressive: i32,
    pub defensive: i32
}

pub trait Evaluator {
    fn name(&self) -> String;
    fn evaluate(
        &self, lock: &LockResult, board: &Board, move_time: u32, piece: Piece
    ) -> Evaluation;
    fn search_options(&self) -> SearchOptions;
}

impl Eval {
    pub fn value(self, height: i32, options: SearchOptions) -> i32 {
        if height <= options.aggressive_height {
            self.aggressive
        } else if height >= options.defensive_height {
            self.defensive
        } else {
            let range_size = options.aggressive_height - options.defensive_height;
            let t = height - options.aggressive_height;
            (self.defensive * t + self.aggressive * (range_size - t)) / range_size
        }
    }
}

impl std::ops::Add<Evaluation> for Eval {
    type Output = Self;
    fn add(self, e: Evaluation) -> Self {
        Eval {
            aggressive: self.aggressive + e.aggressive_accumulated,
            defensive: self.defensive + e.defensive_accumulated
        }
    }
}

impl std::ops::Mul<i32> for Eval {
    type Output = Self;
    fn mul(self, s: i32) -> Self {
        Eval {
            aggressive: self.aggressive * s,
            defensive: self.defensive * s
        }
    }
}

impl std::ops::Div<i32> for Eval {
    type Output = Self;
    fn div(self, s: i32) -> Self {
        Eval {
            aggressive: self.aggressive / s,
            defensive: self.defensive / s
        }
    }
}

impl From<Evaluation> for Eval {
    fn from(v: Evaluation) -> Eval {
        Eval {
            aggressive: v.aggressive_accumulated + v.aggressive_transient,
            defensive: v.defensive_accumulated + v.defensive_transient
        }
    }
}