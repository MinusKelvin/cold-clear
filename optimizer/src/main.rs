use serde::{ Serialize, Deserialize };
use bot::evaluation::Standard;
use rand::prelude::*;
use libflate::deflate;
use std::sync::{ Arc, Mutex };
use std::collections::VecDeque;
use std::sync::mpsc::channel;

mod battle;
mod mutate;

use mutate::Mutateable;

const BATTLES: usize = 10;

fn main() {
    let mut population = match std::fs::File::open("pop.json") {
        Ok(file) => serde_json::from_reader(file).unwrap_or_else(|e| {
            eprintln!("pop.json contained invalid data: {}", e);
            new_population()
        }),
        Err(_) => new_population::<Standard>()
    };

    let matchups = Arc::new(Mutex::new((true, VecDeque::new())));
    let (send, game_results) = channel();
    for _ in 0..12 {
        let matchups = matchups.clone();
        let send = send.clone();
        std::thread::spawn(move || {
            loop {
                let (p1, p1_e, p2, p2_e) = {
                    let (active, ref mut queue) = *matchups.lock().unwrap();
                    if !active { break }
                    match queue.pop_front() {
                        Some(v) => v,
                        None => continue
                    }
                };
                if let Some((replay, p1_won)) = battle::do_battle(p1_e, p2_e) {
                    send.send(Some((if p1_won { p1 } else { p2 }, replay))).ok();
                } else {
                    send.send(None).ok();
                }
            }
        });
    }

    loop {
        let mut count = 0;
        {
            let mut matchups = matchups.lock().unwrap();
            for i in 0..population.members.len() {
                for j in 0..population.members.len() {
                    if i == j { continue }
                    for _ in 0..BATTLES {
                        matchups.1.push_back((
                            i, population.members[i].clone(),
                            j, population.members[j].clone()
                        ));
                        count += 1;
                    }
                }
            }
        }

        let mut results = vec![];
        for i in 0..population.members.len() {
            results.push((i, 0));
        }
        for i in 0..count {
            if let Some((winner, replay)) = game_results.recv().unwrap() {
                results[winner].1 += 1;

                let mut encoder = deflate::Encoder::new(
                    std::fs::File::create("recent-game.dat").unwrap()
                );
                bincode::serialize_into(&mut encoder, &replay).unwrap();
                encoder.finish().unwrap();
            }
            if (i+1) % 100 == 0 {
                println!("Completed game {} of {}", i+1, count);
            }
        }

        results.sort_by_key(|(_, score)| -score);
        println!("Gen {} Results:", population.generation);
        for &(num, score) in &results {
            println!("{}: {} wins", population.members[num].name(), score);
        }
        println!();

        let weighted = rand::distributions::WeightedIndex::new(
            results.iter().map(|&(_, v)| v*v + 1)
        ).unwrap();

        let mut new_population = Population {
            generation: population.generation + 1,
            members: vec![]
        };
        for &(i, _) in results.iter() {
            new_population.members.push(population.members[i].clone());
        }
        for i in 5..population.members.len() {
            let p1 = thread_rng().sample(&weighted);
            let mut p2 = p1;
            while p1 == p2 {
                p2 = thread_rng().sample(&weighted);
            }
            new_population.members[i] = Standard::crossover(
                &population.members[p1], &population.members[p2],
                format!("Gen {} #{}", new_population.generation, i-5)
            );
        }

        serde_json::to_writer(std::fs::File::create("pop.json").unwrap(), &new_population).unwrap();

        match std::fs::File::create(format!("best/{}.json", population.generation)) {
            Ok(f) => serde_json::to_writer(
                std::io::BufWriter::new(f),
                &new_population.members[0]
            ).unwrap_or_else(|e| eprintln!("Error saving best of generation: {}", e)),
            Err(e) => eprintln!("Error saving best of generation: {}", e)
        }

        if std::fs::remove_file("end-request").is_ok() {
            break
        }

        population = new_population;
    }

    matchups.lock().unwrap().0 = false;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Population<E: Mutateable> {
    generation: usize,
    members: Vec<E>
}

fn new_population<E: Mutateable>() -> Population<E> {
    let mut members = vec![];
    for num in 0..20 {
        members.push(E::generate(format!("Gen 0 #{}", num)));
    }
    Population {
        generation: 0,
        members
    }
}