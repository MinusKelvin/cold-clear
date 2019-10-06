use std::collections::VecDeque;
use serde::{ Serialize, Deserialize };
use battle::{ Replay, Battle };
use bot::evaluation::Evaluator;
use rand::prelude::*;
use statrs::distribution::{ Binomial, Univariate };

mod input;
use input::BotInput;

fn main() {
    use bot::evaluation::Standard;
    use bot::evaluation::changed;

    let p1_eval = Standard::default();

    let p2_eval = changed::Standard::default();

    let (send, recv) = std::sync::mpsc::channel();

    for _ in 0..3 {
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

    loop {
        match recv.recv() {
            Ok((replay, p1_won)) => {
                if p1_won {
                    p1_wins += 1;
                } else {
                    p2_wins += 1;
                }
                let games = p1_wins + p2_wins;
                println!("{}-{} ({} games)", p1_wins, p2_wins, games);
                let distr = Binomial::new(0.5, p1_wins + p2_wins).unwrap();
                if p1_wins > p2_wins {
                    let p = distr.cdf(p1_wins as f64 - 1.0);
                    if p > 0.99995 {
                        println!("P(L > R): >99.99%");
                    } else {
                        println!("P(L > R): {:.2}%", p*100.0);
                    }
                } else if p2_wins > p1_wins {
                    let p = distr.cdf(p2_wins as f64 - 1.0);
                    if p > 0.99995 {
                        println!("P(L < R): >99.99%");
                    } else {
                        println!("P(L < R): {:.2}%", p*100.0);
                    }
                } else {
                    println!();
                    println!();
                }

                let f = std::fs::File::create("recent-game.json").unwrap();
                serde_json::to_writer(f, &replay).unwrap();
            },
            Err(_) => break
        }

        if std::fs::remove_file("end-request").is_ok() {
            break
        }
    }
}

fn do_battle(p1: impl Evaluator, p2: impl Evaluator) -> (InfoReplay, bool) {
    let mut battle = Battle::new(
        Default::default(), thread_rng().gen(), thread_rng().gen(), thread_rng().gen()
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
        p1_info_updates.push_back(p1.update(&battle.player_1.board, &update.player_1.events));
        p2_info_updates.push_back(p2.update(&battle.player_2.board, &update.player_2.events));

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
    pub p1_info_updates: VecDeque<Option<bot::Info>>,
    pub p2_info_updates: VecDeque<Option<bot::Info>>
}