use libtetris::{ LockResult, Board, Info };

mod misalike;
pub use self::misalike::MisalikeEvaluator;
mod naive;
pub use self::naive::NaiveEvaluator;
mod pattern;
pub use self::pattern::PatternEvaluator;

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Evaluation {
    pub accumulated: i32,
    pub transient: i32
}

pub trait Evaluator {
    fn info(&self) -> Info;
    fn evaluate(&mut self, lock: &LockResult, board: &Board, soft_dropped: bool) -> Evaluation;
}
