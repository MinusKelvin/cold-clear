use serde::{ Serialize, Deserialize };
use bot::evaluation::PatternEvaluator;
use rand::prelude::*;

mod battle;
mod mutate;

use crate::mutate::Mutateable;

fn main() {
    let mut population = match std::fs::File::open("pop.json") {
        Ok(file) => match serde_json::from_reader(std::io::BufReader::new(file)) {
            Ok(pop) => Some(pop),
            Err(e) => {
                eprintln!("Couldn't load population from pop.json: {}", e);
                None
            }
        }
        Err(e) => {
            eprintln!("Couldn't open pop.json: {}", e);
            None
        }
    }.unwrap_or_else(gen_population::<PatternEvaluator>);

    loop {
        let seeds: [(_, _); 5] = thread_rng().gen();
        let mut matchups = vec![];
        for i in 0..population.individuals.len() {
            for j in 0..population.individuals.len() {
                if i == j { continue }
                for &seed_pair in &seeds {
                    matchups.push((i, j, seed_pair));
                }
            }
        }

        let mut results: Vec<_> = battle::playout(&population, matchups)
            .into_iter()
            .enumerate()
            .collect();
        results.sort_by_key(|v| -(v.1 as isize));
        println!("Gen {} Results:", population.generation);
        for &(num, score) in &results {
            println!("{}: {} wins", population.individuals[num].1, score);
        }
        println!();

        let weighted = rand::distributions::WeightedIndex::new(
            results.iter().map(|&(_, v)| v*v)
        ).unwrap();

        let mut new_population = population.clone();
        for (i, &(ri, _)) in results.iter().enumerate() {
            new_population.individuals[i] = population.individuals[ri].clone();
        }
        new_population.generation += 1;
        for i in 5..20 {
            let p1 = thread_rng().sample(&weighted);
            let mut p2 = p1;
            while p1 == p2 {
                p2 = thread_rng().sample(&weighted);
            }
            new_population.individuals[i] = (PatternEvaluator::crossover(
                &population.individuals[p1].0, &population.individuals[p2].0
            ), IndividualName { gen: new_population.generation, num: i-5 });
        }

        serde_json::to_writer(std::fs::File::create("pop.json").unwrap(), &new_population).unwrap();

        match std::fs::File::create(format!("best/{}.json", population.generation)) {
            Ok(f) => serde_json::to_writer(
                std::io::BufWriter::new(f),
                &new_population.individuals[0].0
            ).unwrap_or_else(|e| eprintln!("Error saving best of generation: {}", e)),
            Err(e) => eprintln!("Error saving best of generation: {}", e)
        }

        if std::fs::remove_file("end-request").is_ok() {
            return
        }

        population = new_population;
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Population<E> {
    generation: usize,
    individuals: Vec<(E, IndividualName)>
}

fn gen_population<E: Mutateable>() -> Population<E> {
    let mut individuals = vec![];
    for num in 0..20 {
        individuals.push((E::gen_random(), IndividualName { gen: 0, num }));
    }
    Population {
        generation: 0,
        individuals
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct IndividualName {
    gen: usize,
    num: usize
}

impl std::fmt::Display for IndividualName {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Gen {} #{}", self.gen, self.num)
    }
}