use libtetris::{ LockResult, Board };

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Evaluation {
    pub accumulated: i32,
    pub transient: i32
}

pub trait Evaluator {
    const NAME: &'static str;

    fn evaluate(&mut self, lock: &LockResult, board: &Board, soft_dropped: bool) -> Evaluation;
}

mod naive;
pub use self::naive::NaiveEvaluator;