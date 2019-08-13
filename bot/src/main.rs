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
            .map(|&v| v as i64)
            .collect::<ArrayVec<[i64; 12]>>()
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
    let mut times_failed_to_extend = 0;

    loop {
        const PIECE_TIME: std::time::Duration = std::time::Duration::from_millis(0_100);
        if start.elapsed() >= PIECE_TIME || times_failed_to_extend > 20 {
            if let Some((h, m, r, mut t)) = tree.take_best_move() {
                let drawing = display::draw_move(
                    &tree.board,
                    &t.board,
                    &m,
                    t.evaluation,
                    t.depth(), attack, pieces,
                    &r,
                    h
                );
                attack += r.garbage_sent;
                pieces += 1;
                display::write_drawings(&mut std::io::stdout(), &[drawing]).unwrap();
                drawings.push(drawing);
                while t.board.next_queue().count() < QUEUE_SIZE {
                    t.add_next_piece(t.board.generate_next_piece());
                }
                tree = t;
                if tree.evaluation == None || pieces >= 1500 {
                    break
                }
            } else if tree.extensions(MOVEMENT_MODE).is_empty() {
                println!("Dead");
                break
            }
            start = std::time::Instant::now();
            times_failed_to_extend = 0;
        }

        let mut path = VecDeque::new();
        let mut branch = &mut tree;

        loop {
            let branches = branch.branches();
            if branches.is_empty() {
                break
            }

            let min = branches.iter()
                .map(|&idx| branch.branch(idx).2.evaluation.unwrap())
                .min().unwrap();
            let &idx = branches.choose_weighted(
                &mut thread_rng(),
                |&idx| {
                    let e = branch.branch(idx).2.evaluation.unwrap() - min;
                    e*e + 10
                }
            ).unwrap();
            let (mv, _, t) = branch.branch_mut(idx);

            if idx.0 {
                path.push_back(None);
            }
            path.push_back(Some(mv.clone()));
            branch = t;
        }

        let extensions = branch.extensions(MOVEMENT_MODE);
        if extensions.is_empty() {
            times_failed_to_extend += 1;
        } else {
            times_failed_to_extend = 0;

            for (hold, mv) in extensions {
                let mut result = branch.board.clone();
                if hold { result.hold(); }
                let p = result.advance_queue();
                assert!(p == Some(mv.location.kind.0));

                let lock = result.lock_piece(mv.location);
                let leaf = Tree::new(
                    result,
                    &lock,
                    mv.soft_dropped,
                    &transient_weights,
                    &acc_weights
                );
                branch.extend(hold, mv, lock, leaf);
            }
        }

        tree.repropagate(path);
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

    let mut plan  = vec![];
    while let Some((h, mv, r, t)) = tree.take_best_move() {
        plan.push(display::draw_move(
            &tree.board,
            &t.board,
            &mv,
            t.evaluation,
            t.depth(), attack, pieces,
            &r,
            h
        ));
        pieces += 1;
        attack += r.garbage_sent;
        tree = t;
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
