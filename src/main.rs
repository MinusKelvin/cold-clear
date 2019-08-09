use rand::prelude::*;
use std::collections::VecDeque;

mod display;
mod evaluation;
mod moves;
mod tetris;

use crate::tetris::{ BoardState, FallingPiece };
use crate::moves::Move;

fn main() {
    let mut root_board = BoardState::new();
    root_board.next_pieces.extend({
        use crate::tetris::Piece::*;
        &[I, T, L, S, J, O, Z, T, O, I]
    });
    let mut tree = Tree::new(root_board);

    let start = std::time::Instant::now();
    loop {
        if start.elapsed() >= std::time::Duration::from_secs(60) { break }

        let mut path = VecDeque::new();
        let mut branch = &mut tree;

        while !branch.children.is_empty() {
            let i = thread_rng().gen_range(0, branch.children.len() * branch.children.len());
            let i = branch.children.len() - 1 - (i as f64).sqrt() as usize;
            path.push_back(branch.children[i].0.clone());
            branch = &mut branch.children[i].1;
        }

        let next = branch.board.get_next_piece_possiblities();
        if next.len() == 1 {
            let mut board = branch.board.clone();

            let piece = next.iter().next().unwrap();
            if !board.next_pieces.is_empty() {
                let p = board.next_pieces.pop_front().unwrap();
                assert!(p == piece);
            }

            if let Some(piece) = FallingPiece::spawn(piece, &board) {
                for mv in moves::find_moves(&board, piece, true) {
                    let mut result = board.clone();
                    result.lock_piece(mv.location);
                    branch.children.push((mv, Tree::new(result)));
                }
            } else {
                branch.evaluation = -1000000;
            }
        }
        drop(branch);
        tree.repropagate(path);
    }

    let best: Vec<_> = tree.best_sequence()
        .into_iter()
        .map(|(mv, board, evaluation)| display::draw_move(board, mv, evaluation))
        .collect();

    display::write_drawings(&mut std::io::stdout(), &best).unwrap();
}

struct Tree {
    board: BoardState,
    evaluation: i32,
    children: Vec<(Move, Tree)>
}

impl Tree {
    fn new(board: BoardState) -> Self {
        Tree {
            evaluation: evaluation::evaluate(&board),
            board,
            children: vec![]
        }
    }

    fn descend<T>(
        &mut self,
        mut path: VecDeque<Move>,
        data: T,
        last: impl FnOnce(&mut Self, T),
        descend: impl FnOnce(&mut Self, VecDeque<Move>, usize, T)
    ) {
        if let Some(mv) = path.pop_front() {
            for i in 0..self.children.len() {
                if self.children[i].0 == mv {
                    descend(self, path, i, data);
                    return
                }
            }
        } else {
            last(self, data)
        }
    }

    fn repropagate(&mut self, path: VecDeque<Move>) {
        self.descend(
            path, (),
            |this, _| {
                if !this.children.is_empty() {
                    this.children.sort_by_key(|(_, t)| t.evaluation);
                    this.evaluation = this.children[0].1.evaluation;
                }
            },
            |this, path, i, _| {
                this.children[i].1.repropagate(path);
                ensure_sorted(&mut this.children, i);
                this.evaluation = this.children[0].1.evaluation;
            }
        );
    }

    fn best_sequence(&self) -> Vec<(&Move, &BoardState, i32)> {
        let mut v = vec![];
        self.best_seq_impl(&mut v);
        v
    }

    fn best_seq_impl<'a>(&'a self, into: &mut Vec<(&'a Move, &'a BoardState, i32)>) {
        if let Some((mv, t)) = self.children.first() {
            into.push((mv, &t.board, t.evaluation));
            t.best_seq_impl(into);
        }
    }
}

fn swap_back(list: &mut [(Move, Tree)], i: usize) {
    for i in i..list.len()-1 {
        if list[i].1.evaluation < list[i+1].1.evaluation {
            list.swap(i, i+1);
        } else {
            break
        }
    }
}

fn swap_forward(list: &mut [(Move, Tree)], i: usize) {
    for i in (0..i).rev() {
        if list[i].1.evaluation < list[i+1].1.evaluation {
            list.swap(i, i+1);
        } else {
            break
        }
    }
}

fn ensure_sorted(list: &mut [(Move, Tree)], i: usize) {
    if i == 0 {
        if list[0].1.evaluation < list[1].1.evaluation {
            swap_back(list, i);
        }
    } else if list[i-1].1.evaluation < list[i].1.evaluation {
        swap_forward(list, i);
    } else {
        swap_back(list, i);
    }
}