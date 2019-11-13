use serde::{ Serialize, Deserialize };
use bot::evaluation::Standard;
use rand::prelude::*;

mod battle;
mod mutate;

use mutate::Mutateable;

const BATTLES: usize = 2;

fn main() {
    let mut population = match std::fs::File::open("pop.json") {
        Ok(file) => serde_json::from_reader(file).unwrap_or_else(|e| {
            eprintln!("pop.json contained invalid data: {}", e);
            new_population()
        }),
        Err(_) => new_population::<Standard>()
    };

    let (replay_saver, recv) = crossbeam_channel::unbounded();
    let replay_save_thread = std::thread::spawn(move || {
        while let Ok(replay) = recv.recv() {
            if let Ok(f) = std::fs::File::create("recent-game.json") {
                serde_json::to_writer(std::io::BufWriter::new(f), &replay).ok();
            }
        }
    });

    let (matchups, recv) = crossbeam_channel::unbounded();
    let (send, game_results) = crossbeam_channel::unbounded();
    for _ in 0..12 {
        let recv = recv.clone();
        let send = send.clone();
        let replay_saver = replay_saver.clone();
        std::thread::spawn(move || {
            while let Ok((p1, p1_e, p2, p2_e)) = recv.recv() {
                if let Some((replay, p1_won)) = battle::do_battle(p1_e, p2_e) {
                    replay_saver.send(replay).ok();
                    send.send(Some(if p1_won { p1 } else { p2 })).ok();
                } else {
                    send.send(None).ok();
                }
            }
        });
    }

    loop {
        let mut count = 0;
        for i in 0..population.members.len() {
            for j in 0..population.members.len() {
                if i == j { continue }
                for _ in 0..BATTLES {
                    matchups.send((
                        i, population.members[i].clone(),
                        j, population.members[j].clone()
                    )).ok();
                    count += 1;
                }
            }
        }

        let mut results = vec![];
        for i in 0..population.members.len() {
            results.push((i, 0));
        }
        for i in 0..count {
            if let Some(winner) = game_results.recv().unwrap() {
                results[winner].1 += 1;
            }
            println!("Completed game {} of {}", i, count);
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

    drop(replay_saver);
    replay_save_thread.join().ok();
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