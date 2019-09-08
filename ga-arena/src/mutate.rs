use rand::prelude::*;
use bot::evaluation::PatternEvaluator;

pub trait Mutateable {
    fn gen_random() -> Self;

    fn crossover(parent1: &Self, parent2: &Self) -> Self;
}

impl Mutateable for PatternEvaluator {
    fn gen_random() -> Self {
        let mut individual = PatternEvaluator {
            back_to_back: thread_rng().gen_range(-999, 1000),
            thresholds: [0; 4096].into(),
            below_values: [0; 4096].into(),
            above_values: [0; 4096].into(),

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
            combo_table: [0; 12],
            soft_drop: thread_rng().gen_range(-999, 1000)
        };
        for i in 0..4096 {
            individual.thresholds[i] = thread_rng().gen_range(0, 250);
            individual.above_values[i] = thread_rng().gen_range(-999, 1000);
            individual.below_values[i] = thread_rng().gen_range(-999, 1000);
        }
        for i in 0..12 {
            individual.combo_table[i] = thread_rng().gen_range(-999, 1000);
        }
        individual
    }

    fn crossover(parent1: &Self, parent2: &Self) -> Self {
        let mut this = PatternEvaluator {
            back_to_back: crossover_gene(parent1.back_to_back, parent2.back_to_back, -999, 999, 10),
            thresholds: [0; 4096].into(),
            below_values: [0; 4096].into(),
            above_values: [0; 4096].into(),

            b2b_clear: crossover_gene(parent1.b2b_clear, parent2.b2b_clear, -999, 999, 10),
            clear1: crossover_gene(parent1.clear1, parent2.clear1, -999, 999, 10),
            clear2: crossover_gene(parent1.clear2, parent2.clear2, -999, 999, 10),
            clear3: crossover_gene(parent1.clear3, parent2.clear3, -999, 999, 10),
            clear4: crossover_gene(parent1.clear4, parent2.clear4, -999, 999, 10),
            tspin1: crossover_gene(parent1.tspin1, parent2.tspin1, -999, 999, 10),
            tspin2: crossover_gene(parent1.tspin2, parent2.tspin2, -999, 999, 10),
            tspin3: crossover_gene(parent1.tspin3, parent2.tspin3, -999, 999, 10),
            mini_tspin1: crossover_gene(parent1.mini_tspin1, parent2.mini_tspin1, -999, 999, 10),
            mini_tspin2: crossover_gene(parent1.mini_tspin2, parent2.mini_tspin2, -999, 999, 10),
            perfect_clear: crossover_gene(parent1.perfect_clear, parent2.perfect_clear, -999, 999, 10),
            combo_table: [0; 12],
            soft_drop: crossover_gene(parent1.soft_drop, parent2.soft_drop, -999, 999, 10)
        };
        for i in 0..4096 {
            this.thresholds[i] = crossover_gene(
                parent1.thresholds[i], parent2.thresholds[i], 0, 250, 1
            );
            this.above_values[i] = crossover_gene(
                parent1.above_values[i], parent2.above_values[i], -999, 999, 10
            );
            this.below_values[i] = crossover_gene(
                parent1.below_values[i], parent2.below_values[i], -999, 999, 10
            );
        }
        for i in 0..12 {
            this.combo_table[i] = crossover_gene(
                parent1.combo_table[i], parent2.combo_table[i], -999, 999, 10
            );
        }
        this
    }
}

fn crossover_gene(v1: i32, v2: i32, min: i32, max: i32, variance: i32) -> i32 {
    let v = match thread_rng().gen_range(0, 100) {
        0...41 => v1, // 42%
        42...83 => v2, // 42%
        84...98 => (v1 + v2) / 2, // 15%
        _ => thread_rng().gen_range(min, max+1)
    } + thread_rng().gen_range(-variance, variance+1);
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}