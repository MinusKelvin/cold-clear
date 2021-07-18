use cold_clear::evaluation::Standard;
use rand::prelude::*;

pub trait Mutateable: Default {
    fn generate(sub_name: String) -> Self;

    fn crossover(parent1: &Self, parent2: &Self, sub_name: String) -> Self;

    fn name(&self) -> &str;
}

impl Mutateable for Standard {
    fn generate(sub_name: String) -> Self {
        Standard {
            #[cfg(feature = "tetrio_garbage")]
            b2b_chain: thread_rng().gen_range(-999, 1000),
            #[cfg(feature = "tetrio_garbage")]
            b2b_chain_log: thread_rng().gen_range(-999, 1000),
            #[cfg(feature = "tetrio_garbage")]
            combo_multiplier: thread_rng().gen_range(-999, 1000),
            back_to_back: thread_rng().gen_range(-999, 1000),
            bumpiness: thread_rng().gen_range(-999, 1000),
            bumpiness_sq: thread_rng().gen_range(-999, 1000),
            row_transitions: thread_rng().gen_range(-999, 1000),
            height: thread_rng().gen_range(-999, 1000),
            top_half: thread_rng().gen_range(-999, 1000),
            top_quarter: thread_rng().gen_range(-999, 1000),
            jeopardy: thread_rng().gen_range(-999, 1000),
            cavity_cells: thread_rng().gen_range(-999, 1000),
            cavity_cells_sq: thread_rng().gen_range(-999, 1000),
            overhang_cells: thread_rng().gen_range(-999, 1000),
            overhang_cells_sq: thread_rng().gen_range(-999, 1000),
            covered_cells: thread_rng().gen_range(-999, 1000),
            covered_cells_sq: thread_rng().gen_range(-999, 1000),
            tslot: [
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
            ],
            well_depth: thread_rng().gen_range(-999, 1000),
            max_well_depth: thread_rng().gen_range(-999, 1000),
            well_column: [
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
                thread_rng().gen_range(-999, 1000),
            ],

            move_time: thread_rng().gen_range(-999, 1000),
            wasted_t: thread_rng().gen_range(-999, 1000),
            b2b_clear: thread_rng().gen_range(-999, 1000),
            clear1: thread_rng().gen_range(-999, 1000),
            clear2: thread_rng().gen_range(-999, 1000),
            clear3: thread_rng().gen_range(-999, 1000),
            clear4: thread_rng().gen_range(-999, 1000),
            tspin1: thread_rng().gen_range(-999, 1000),
            tspin2: thread_rng().gen_range(-999, 1000),
            tspin3: thread_rng().gen_range(-999, 1000),
            mini_tspin1: thread_rng().gen_range(-999, 1000),
            mini_tspin2: thread_rng().gen_range(-999, 1000),
            perfect_clear: thread_rng().gen_range(-999, 1000),
            combo_garbage: thread_rng().gen_range(-999, 1000),

            use_bag: true,
            timed_jeopardy: true,
            stack_pc_damage: cfg!(feature = "tetrio_garbage"),
            sub_name: Some(sub_name),
        }
    }

    fn crossover(parent1: &Self, parent2: &Self, sub_name: String) -> Self {
        Standard {
            #[cfg(feature = "tetrio_garbage")]
            b2b_chain: crossover_gene(parent1.b2b_chain, parent2.b2b_chain),
            #[cfg(feature = "tetrio_garbage")]
            b2b_chain_log: crossover_gene(parent1.b2b_chain_log, parent2.b2b_chain_log),
            #[cfg(feature = "tetrio_garbage")]
            combo_multiplier: crossover_gene(parent1.combo_multiplier, parent2.combo_multiplier),
            back_to_back: crossover_gene(parent1.back_to_back, parent2.back_to_back),
            bumpiness: crossover_gene(parent1.bumpiness, parent2.bumpiness),
            bumpiness_sq: crossover_gene(parent1.bumpiness_sq, parent2.bumpiness_sq),
            row_transitions: crossover_gene(parent1.row_transitions, parent2.row_transitions),
            height: crossover_gene(parent1.height, parent2.height),
            top_half: crossover_gene(parent1.top_half, parent2.top_half),
            top_quarter: crossover_gene(parent1.top_quarter, parent2.top_quarter),
            jeopardy: crossover_gene(parent1.jeopardy, parent2.jeopardy),
            cavity_cells: crossover_gene(parent1.cavity_cells, parent2.cavity_cells),
            cavity_cells_sq: crossover_gene(parent1.cavity_cells_sq, parent2.cavity_cells_sq),
            overhang_cells: crossover_gene(parent1.overhang_cells, parent2.overhang_cells),
            overhang_cells_sq: crossover_gene(parent1.overhang_cells_sq, parent2.overhang_cells_sq),
            covered_cells: crossover_gene(parent1.covered_cells, parent2.covered_cells),
            covered_cells_sq: crossover_gene(parent1.covered_cells_sq, parent2.covered_cells_sq),
            tslot: [
                crossover_gene(parent1.tslot[0], parent2.tslot[0]),
                crossover_gene(parent1.tslot[1], parent2.tslot[1]),
                crossover_gene(parent1.tslot[2], parent2.tslot[2]),
                crossover_gene(parent1.tslot[3], parent2.tslot[3]),
            ],
            well_depth: crossover_gene(parent1.well_depth, parent2.well_depth),
            max_well_depth: crossover_gene(parent1.max_well_depth, parent2.max_well_depth),
            well_column: [
                crossover_gene(parent1.well_column[0], parent2.well_column[0]),
                crossover_gene(parent1.well_column[1], parent2.well_column[1]),
                crossover_gene(parent1.well_column[2], parent2.well_column[2]),
                crossover_gene(parent1.well_column[3], parent2.well_column[3]),
                crossover_gene(parent1.well_column[4], parent2.well_column[4]),
                crossover_gene(parent1.well_column[5], parent2.well_column[5]),
                crossover_gene(parent1.well_column[6], parent2.well_column[6]),
                crossover_gene(parent1.well_column[7], parent2.well_column[7]),
                crossover_gene(parent1.well_column[8], parent2.well_column[8]),
                crossover_gene(parent1.well_column[9], parent2.well_column[9]),
            ],

            move_time: crossover_gene(parent1.move_time, parent2.move_time),
            wasted_t: crossover_gene(parent1.wasted_t, parent2.wasted_t),
            b2b_clear: crossover_gene(parent1.b2b_clear, parent2.b2b_clear),
            clear1: crossover_gene(parent1.clear1, parent2.clear1),
            clear2: crossover_gene(parent1.clear2, parent2.clear2),
            clear3: crossover_gene(parent1.clear3, parent2.clear3),
            clear4: crossover_gene(parent1.clear4, parent2.clear4),
            tspin1: crossover_gene(parent1.tspin1, parent2.tspin1),
            tspin2: crossover_gene(parent1.tspin2, parent2.tspin2),
            tspin3: crossover_gene(parent1.tspin3, parent2.tspin3),
            mini_tspin1: crossover_gene(parent1.mini_tspin1, parent2.mini_tspin1),
            mini_tspin2: crossover_gene(parent1.mini_tspin2, parent2.mini_tspin2),
            perfect_clear: crossover_gene(parent1.perfect_clear, parent2.perfect_clear),
            combo_garbage: crossover_gene(parent1.combo_garbage, parent2.combo_garbage),

            use_bag: true,
            timed_jeopardy: true,
            stack_pc_damage: cfg!(feature = "tetrio_garbage"),
            sub_name: Some(sub_name),
        }
    }

    fn name(&self) -> &str {
        self.sub_name.as_ref().map(|s| &**s).unwrap_or("")
    }
}

fn crossover_gene(v1: i32, v2: i32) -> i32 {
    let v = match thread_rng().gen_range(0, 100) {
        0..=41 => v1,             // 42%
        42..=83 => v2,            // 42%
        84..=98 => (v1 + v2) / 2, // 15%
        _ => thread_rng().gen_range(-999, 1000),
    } + thread_rng().gen_range(-10, 11);
    if v < -999 {
        -999
    } else if v > 999 {
        999
    } else {
        v
    }
}
