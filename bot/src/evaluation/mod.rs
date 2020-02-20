use libtetris::{ LockResult, Board, Piece };
use crate::tree::MoveCandidate;

mod misalike;
pub use self::misalike::Misalike;
mod standard;
pub use self::standard::Standard;
pub mod changed;

pub trait Evaluator : Send + Sync {
    type Value: Evaluation<Self::Reward> + Send;
    type Reward: Clone + Send;

    fn name(&self) -> String;

    fn evaluate(
        &self, lock: &LockResult, board: &Board, move_time: u32, placed: Piece
    ) -> (Self::Value, Self::Reward);

    fn pick_move(
        &self, candidates: Vec<MoveCandidate<Self::Value>>, _incoming: u32
    ) -> MoveCandidate<Self::Value> {
        candidates.into_iter().next().unwrap()
    }
}

pub trait Evaluation<R> : Eq + Ord + Default + Clone
    + std::ops::Add<R, Output=Self>
    + std::ops::Div<usize, Output=Self>
    + std::ops::Mul<usize, Output=Self>
    + std::ops::Add<Output=Self>
{
    fn modify_death(self) -> Self;
    fn weight(self, min: &Self, rank: usize) -> i64;
}