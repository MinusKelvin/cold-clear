use libtetris::{Board, LockResult, Piece};

use crate::dag::MoveCandidate;

mod standard;
pub use self::standard::Standard;
pub mod changed;

pub trait Evaluator: Send + Sync {
    type Value: Evaluation<Self::Reward> + Send + 'static;
    type Reward: Clone + Send + 'static;

    fn name(&self) -> String;

    fn evaluate(
        &self,
        lock: &LockResult,
        board: &Board,
        move_time: u32,
        placed: Piece,
    ) -> (Self::Value, Self::Reward);

    fn pick_move(
        &self,
        candidates: Vec<MoveCandidate<Self::Value>>,
        _incoming: u32,
    ) -> MoveCandidate<Self::Value> {
        candidates.into_iter().next().unwrap()
    }
}

pub trait Evaluation<R>:
    Eq
    + Ord
    + Default
    + Clone
    + std::ops::Add<R, Output = Self>
    + std::ops::Div<usize, Output = Self>
    + std::ops::Mul<usize, Output = Self>
    + std::ops::Add<Output = Self>
{
    fn modify_death(self) -> Self;
    fn weight(self, min: &Self, rank: usize) -> i64;

    fn improve(&mut self, other: Self);
}

impl<T: Evaluator> Evaluator for std::sync::Arc<T> {
    type Value = T::Value;
    type Reward = T::Reward;

    fn name(&self) -> String {
        (**self).name()
    }

    fn evaluate(
        &self,
        lock: &LockResult,
        board: &Board,
        move_time: u32,
        placed: Piece,
    ) -> (T::Value, T::Reward) {
        (**self).evaluate(lock, board, move_time, placed)
    }

    fn pick_move(
        &self,
        candidates: Vec<MoveCandidate<Self::Value>>,
        incoming: u32,
    ) -> MoveCandidate<Self::Value> {
        (**self).pick_move(candidates, incoming)
    }
}
