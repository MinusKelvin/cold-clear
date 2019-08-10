use rand::prelude::*;
use std::collections::VecDeque;

mod display;
mod evaluation;
mod moves;
mod tetris;

use crate::tetris::{ BoardState, FallingPiece, LockResult };
use crate::moves::Move;

fn main() {
    let weights = evaluation::Weights {
        back_to_back: 50,
        bumpiness: -1,
        bumpiness_sq: -5,
        height: 1,
        top_half: -5,
        top_quarter: -20,
        cavity_cells: -50,
        cavity_cells_sq: -10,
        overhang_cells: -10,
        overhang_cells_sq: -10,
        covered_cells: -10,
        covered_cells_sq: -1
    };

    let mut root_board = BoardState::new();
    root_board.add_next_piece(root_board.generate_next_piece());
    root_board.add_next_piece(root_board.generate_next_piece());
    root_board.add_next_piece(root_board.generate_next_piece());
    root_board.add_next_piece(root_board.generate_next_piece());
    root_board.add_next_piece(root_board.generate_next_piece());
    root_board.add_next_piece(root_board.generate_next_piece());
    let mut tree = Tree::new(root_board, &weights);

    let mut drawings = vec![];

    let mut start = std::time::Instant::now();
    let mut times_failed_to_extend = 0;

    loop {
        const PIECE_TIME: std::time::Duration = std::time::Duration::from_millis(500);
        if start.elapsed() >= PIECE_TIME || times_failed_to_extend > 20 {
            if let Some((m, r, t)) = tree.take_best_move() {
                let drawing = display::draw_move(&tree.board, &m, t.evaluation, t.depth(), r);
                display::write_drawings(&mut std::io::stdout(), &[drawing]).unwrap();
                drawings.push(drawing);
                tree = t;
                tree.add_next_piece(tree.board.generate_next_piece());
                if tree.evaluation == None || tree.board.piece_count >= 1000 {
                    break
                }
            } else {
                let next = tree.board.get_next_piece_possiblities();
                if next.len() == 1 {
                    let piece = next.iter().next().unwrap();
                    let piece = FallingPiece::spawn(piece, &tree.board);
                    if let Some(piece) = piece {
                        if moves::find_moves(&tree.board, piece, moves::MovementMode::ZeroG).is_empty() {
                            println!("Dead");
                            break
                        }
                    } else {
                        println!("Dead");
                        break
                    }
                }
            }
            start = std::time::Instant::now();
            times_failed_to_extend = 0;
        }

        let mut path = VecDeque::new();
        let mut branch = &mut tree;

        while branch.children.iter().filter_map(|(_, _, t)| t.evaluation).next().is_some() {
            let min = branch.children.iter().filter_map(|(_, _, t)| t.evaluation).min().unwrap();
            let chosen = branch.children.choose_weighted_mut(
                &mut thread_rng(),
                |(_, _, t)| t.evaluation.map_or(0, |v| v - min + 10)
            ).unwrap();
            path.push_back(chosen.0.clone());
            branch = &mut chosen.2;
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
                for mv in moves::find_moves(&board, piece, moves::MovementMode::ZeroGFinesse) {
                    let mut result = board.clone();
                    let lock = result.lock_piece(mv.location);
                    branch.children.push((mv, lock, Tree::new(result, &weights)));
                }
                if branch.children.is_empty() {
                    branch.evaluation = None;
                    times_failed_to_extend += 1;
                } else {
                    times_failed_to_extend = 0;
                }
            } else {
                branch.evaluation = None;
                times_failed_to_extend += 1;
            }
        } else {
            times_failed_to_extend += 1;
        }
        drop(branch);
        tree.repropagate(path);
    }

    unsafe {
        println!("Found a total of {} moves in {:?}", moves::MOVES_FOUND, moves::TIME_TAKEN);
        println!("That's one move every {:?}", moves::TIME_TAKEN / moves::MOVES_FOUND as u32);
        println!();
        println!("Evaluated a total of {} boards in {:?}",
            evaluation::BOARDS_EVALUATED, evaluation::TIME_TAKEN);
        println!("That's one board every {:?}",
            evaluation::TIME_TAKEN / evaluation::BOARDS_EVALUATED as u32);
    }

    let best = tree.best_sequence();
    let best_len = best.len();
    let best: Vec<_> = best.into_iter()
        .enumerate()
        .map(|(i, (mv, r, board, evaluation))|
            display::draw_move(board, mv, evaluation, (best_len - i) as u32 - 1, *r)
        )
        .collect();

    println!("Plan:");
    display::write_drawings(&mut std::io::stdout(), &best).unwrap();

    display::write_drawings(&mut std::fs::File::create("playout").unwrap(), &drawings).unwrap();
}

struct Tree {
    board: BoardState,
    evaluation: Option<i32>,
    children: Vec<(Move, LockResult, Tree)>
}

impl Tree {
    fn new(board: BoardState, weight: &evaluation::Weights) -> Self {
        Tree {
            evaluation: Some(evaluation::evaluate(&board, weight)),
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
                let best = this.children.iter().map(|(_, _, t)| t.evaluation).max();
                if let Some(best) = best {
                    this.evaluation = best;
                }
            },
            |this, path, i, _| {
                this.children[i].2.repropagate(path);
                let best = this.children.iter().map(|(_, _, t)| t.evaluation).max();
                if let Some(best) = best {
                    this.evaluation = best;
                }
            }
        );
    }

    fn best_sequence(&self) -> Vec<(&Move, &LockResult, &BoardState, Option<i32>)> {
        let mut v = vec![];
        self.best_seq_impl(&mut v);
        v
    }

    fn best_index(&self) -> Option<usize> {
        let mut best: Option<usize> = None;
        for i in 0..self.children.len() {
            if let Some(b) = best {
                if let Some(s) = self.children[i].2.evaluation {
                    if self.children[b].2.evaluation.unwrap() < s {
                        best = Some(i);
                    }
                }
            } else if self.children[i].2.evaluation.is_some() {
                best = Some(i);
            }
        }
        best
    }

    fn best_seq_impl<'a>(
        &'a self,
        into: &mut Vec<(&'a Move, &'a LockResult, &'a BoardState, Option<i32>)>
    ) {
        let best = self.best_index();
        if let Some((mv, r, t)) = best.map(|i| &self.children[i]) {
            into.push((mv, r, &self.board, t.evaluation));
            t.best_seq_impl(into);
        }
    }

    fn take_best_move(&mut self) -> Option<(Move, LockResult, Tree)> {
        self.best_index().map(|i| self.children.remove(i))
    }

    fn add_next_piece(&mut self, piece: tetris::Piece) {
        self.board.add_next_piece(piece);
        for (_, _, t) in &mut self.children {
            t.add_next_piece(piece);
        }
    }

    fn depth(&self) -> u32 {
        if let Some(i) = self.best_index() {
            self.children[i].2.depth() + 1
        } else {
            0
        }
    }
}