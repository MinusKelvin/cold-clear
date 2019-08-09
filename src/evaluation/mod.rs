use crate::tetris::BoardState;

pub fn evaluate(board: &BoardState) -> i32 {
    board.total_garbage as i32 * 100
}