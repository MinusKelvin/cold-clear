use std::collections::VecDeque;
use serde::{ Serialize, Deserialize };
use battle::{ Replay, Battle, GameConfig };
use cold_clear::evaluation::Evaluator;
use rand::prelude::*;
use statrs::distribution::{ Binomial, Univariate };
use libflate::deflate;

mod input;
use input::BotInput;

fn main() {
    use cold_clear::evaluation::Standard;
    use cold_clear::evaluation::changed;

    let p1_eval = Standard::fast_config();

    let p2_eval = changed::Standard::fast_config();

    let (send, recv) = std::sync::mpsc::channel();

    for _ in 0..12 {
        let p1_eval = p1_eval.clone();
        let p2_eval = p2_eval.clone();
        let send = send.clone();
        std::thread::spawn(move || loop {
            if send.send(do_battle(p1_eval.clone(), p2_eval.clone())).is_err() {
                break
            };
        });
    }

    let mut p1_wins = 0;
    let mut p2_wins = 0;

    let games = 10000;

    while p1_wins + p2_wins < games {
        match recv.recv() {
            Ok((replay, p1_won)) => {
                if p1_won {
                    p1_wins += 1;
                } else {
                    p2_wins += 1;
                }

                let mut encoder = deflate::Encoder::new(
                    std::fs::File::create("recent-game.dat"
                ).unwrap());
                bincode::serialize_into(&mut encoder, &replay).unwrap();
                encoder.finish().unwrap();

                println!("{} of {}", p1_wins + p2_wins, games);
                println!("{} - {}", p1_wins, p2_wins);
            },
            Err(_) => break
        }
    }
    let distr = Binomial::new(0.5, p1_wins + p2_wins).unwrap();
    let p = distr.cdf(p1_wins as f64);
    println!("p = {:.4}", p);
}

fn do_battle(p1: impl Evaluator + Clone, p2: impl Evaluator + Clone) -> (InfoReplay, bool) {
    let mut battle = Battle::new(
        GameConfig::fast_config(), GameConfig::fast_config(),
        thread_rng().gen(), thread_rng().gen(), thread_rng().gen()
    );

    battle.replay.p1_name = format!("Cold Clear\n{}", p1.name());
    battle.replay.p2_name = format!("Cold Clear\n{}", p2.name());

    let mut p1 = BotInput::new(battle.player_1.board.to_compressed(), p1);
    let mut p2 = BotInput::new(battle.player_2.board.to_compressed(), p2);

    let mut p1_info_updates = VecDeque::new();
    let mut p2_info_updates = VecDeque::new();

    let p1_won;
    'battle: loop {
        let update = battle.update(p1.controller, p2.controller);
        p1_info_updates.push_back(p1.update(
            &battle.player_1.board, &update.player_1.events,
            battle.player_1.garbage_queue
        ));
        p2_info_updates.push_back(p2.update(
            &battle.player_2.board, &update.player_2.events,
            battle.player_2.garbage_queue
        ));

        for event in &update.player_1.events {
            use battle::Event::*;
            match event {
                GameOver => {
                    p1_won = false;
                    break 'battle;
                }
                _ => {}
            }
        }
        for event in &update.player_2.events {
            use battle::Event::*;
            match event {
                GameOver => {
                    p1_won = true;
                    break 'battle;
                }
                _ => {}
            }
        }
    }

    for _ in 0..180 {
        battle.replay.updates.push_back(Default::default());
        p1_info_updates.push_back(None);
        p2_info_updates.push_back(None);
    }

    (InfoReplay {
        replay: battle.replay,
        p1_info_updates,
        p2_info_updates
    }, p1_won)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InfoReplay {
    pub replay: Replay,
    pub p1_info_updates: VecDeque<Option<cold_clear::Info>>,
    pub p2_info_updates: VecDeque<Option<cold_clear::Info>>
}