use rand::prelude::*;
use std::collections::VecDeque;
use arrayvec::ArrayVec;

mod display;
mod evaluation;
mod moves;
mod tree;

use libtetris::Board;
use crate::tree::Tree;

fn main() {
    let transient_weights = evaluation::BoardWeights {
        back_to_back: 50,
        bumpiness: -10,
        bumpiness_sq: -5,
        height: -40,
        top_half: -150,
        top_quarter: -1000,
        cavity_cells: -150,
        cavity_cells_sq: -10,
        overhang_cells: -50,
        overhang_cells_sq: -10,
        covered_cells: -10,
        covered_cells_sq: -10,
        tslot_present: 150,
        well_depth: 50,
        max_well_depth: 8
    };

    let acc_weights = evaluation::PlacementWeights {
        soft_drop: 0,
        b2b_clear: 100,
        clear1: -150,
        clear2: -100,
        clear3: -50,
        clear4: 400,
        tspin1: 130,
        tspin2: 400,
        tspin3: 600,
        mini_tspin1: 0,
        mini_tspin2: 100,
        perfect_clear: 1000,
        combo_table: libtetris::COMBO_GARBAGE.iter()
            .map(|&v| v as i32)
            .collect::<ArrayVec<[_; 12]>>()
            .into_inner()
            .unwrap()
    };

    const MOVEMENT_MODE: crate::moves::MovementMode = crate::moves::MovementMode::ZeroGFinesse;

    let mut root_board = Board::new();
    const QUEUE_SIZE: usize = 6;
    for _ in 0..QUEUE_SIZE {
        root_board.add_next_piece(root_board.generate_next_piece());
    }
    let mut tree = Tree::new(
        root_board,
        &Default::default(),
        false,
        &transient_weights,
        &acc_weights
    );

    let mut drawings = vec![];
    let mut pieces = 0;
    let mut attack = 0;

    let mut start = std::time::Instant::now();

    loop {
        const PIECE_TIME: std::time::Duration = std::time::Duration::from_millis(0_100);
        if start.elapsed() >= PIECE_TIME {
            let b = tree.board.clone();
            match tree.into_best_child() {
                Ok(mut child) => {
                    let drawing = display::draw_move(
                        &b,
                        &child.tree.board,
                        &child.mv,
                        child.tree.evaluation,
                        child.tree.depth as u32, child.tree.child_nodes, attack, pieces,
                        &child.lock,
                        child.hold
                    );
                    attack += child.lock.garbage_sent;
                    pieces += 1;
                    display::write_drawings(&mut std::io::stdout(), &[drawing]).unwrap();
                    drawings.push(drawing);
                    while child.tree.board.next_queue().count() < QUEUE_SIZE {
                        child.tree.add_next_piece(child.tree.board.generate_next_piece());
                    }
                    tree = child.tree;
                    if pieces >= 20000 {
                        break
                    }
                }
                Err(t) => tree = t,
            }
            start = std::time::Instant::now();
        }

        if tree.extend(MOVEMENT_MODE, &transient_weights, &acc_weights) {
            println!("Dead");
            break
        }
    }

    unsafe {
        println!("Found a total of {} moves in {} calls in {:?}", moves::MOVES_FOUND,
            moves::CALLS, moves::TIME_TAKEN_INIT + moves::TIME_TAKEN_ON_STACK);
        println!("That's {:?} per move", (moves::TIME_TAKEN_INIT + moves::TIME_TAKEN_ON_STACK) / moves::MOVES_FOUND as u32);
        println!("That's {:?} per call", (moves::TIME_TAKEN_INIT + moves::TIME_TAKEN_ON_STACK) / moves::CALLS as u32);
        println!("{:?} per call on initialization", moves::TIME_TAKEN_INIT / moves::CALLS as u32);
        println!("{:?} per call on stack movement", moves::TIME_TAKEN_ON_STACK / moves::CALLS as u32);
        println!();
        println!("Evaluated a total of {} boards in {:?}",
            evaluation::BOARDS_EVALUATED, evaluation::TIME_TAKEN);
        println!("That's one board every {:?}",
            evaluation::TIME_TAKEN / evaluation::BOARDS_EVALUATED as u32);
    }

    let mut plan = vec![];
    let mut b = tree.board.clone();
    while let Ok(child) = tree.into_best_child() {
        plan.push(display::draw_move(
            &b,
            &child.tree.board,
            &child.mv,
            child.tree.evaluation,
            child.tree.depth as u32, child.tree.child_nodes, attack, pieces,
            &child.lock,
            child.hold
        ));
        pieces += 1;
        attack += child.lock.garbage_sent;
        tree = child.tree;
        b = tree.board.clone();
    }

    println!("Plan:");
    display::write_drawings(&mut std::io::stdout(), &plan).unwrap();

    let t = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .map_err(|e| -(e.duration().as_secs() as i64))
        .unwrap_or_else(|v| v);
    display::write_drawings(
        &mut std::fs::File::create(format!("playout-{}", t)).unwrap(),
        &drawings
    ).unwrap();
}
