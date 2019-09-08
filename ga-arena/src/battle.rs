use rand::prelude::*;
use rand_pcg::Pcg64Mcg;
use libtetris::{ Event, Replay, Info, Board, LockResult };
use bot::evaluation::{ Evaluator, Evaluation };
use bot::BotController;
use std::collections::VecDeque;
use crate::{ Population, IndividualName };
use std::sync::{ Arc, Mutex };

type Seed = <Pcg64Mcg as SeedableRng>::Seed;

struct Battle {
    p1: BotController,
    p2: BotController,
    p1_num: usize,
    p2_num: usize,
    battle: libtetris::Battle,
}

impl Battle {
    fn new(
        p1: impl Evaluator + Send + 'static,
        p1_num: usize,
        p1_seed: Seed,
        p1_name: IndividualName,

        p2: impl Evaluator + Send + 'static,
        p2_num: usize,
        p2_seed: Seed,
        p2_name: IndividualName
    ) -> Self {
        let battle = libtetris::Battle::new(
            Default::default(), p1_seed, p2_seed, thread_rng().gen()
        );
        Battle {
            p1: BotController::new(
                battle.player_1.board.to_compressed(),
                false,
                GenInfoEvaluator { name: p1_name, eval: p1 }
            ),
            p2: BotController::new(
                battle.player_2.board.to_compressed(),
                false,
                GenInfoEvaluator { name: p2_name, eval: p2 }
            ),
            p1_num, p2_num, battle
        }
    }

    pub fn update(&mut self) -> Option<(Replay, usize)> {
        let p1_controller = self.p1.controller();
        let p2_controller = self.p2.controller();

        let result = self.battle.update(p1_controller, p2_controller);

        let p1_info = self.p1.update(&result.player_1.events, &self.battle.player_1.board);
        self.battle.replay.updates.back_mut().unwrap().1 = p1_info;
        let p2_info = self.p2.update(&result.player_2.events, &self.battle.player_2.board);
        self.battle.replay.updates.back_mut().unwrap().3 = p2_info;

        for event in result.player_1.events {
            match event {
                Event::GameOver => {
                    let mut replay = Replay {
                        p1_seed: [0; 16],
                        p2_seed: [0; 16],
                        garbage_seed: [0; 16],
                        config: Default::default(),
                        updates: VecDeque::new()
                    };
                    std::mem::swap(&mut replay, &mut self.battle.replay);
                    for _ in 0..180 {
                        replay.updates.push_back(Default::default());
                    }
                    return Some((replay, self.p2_num))
                }
                _ => {}
            }
        }

        for event in result.player_2.events {
            match event {
                Event::GameOver => {
                    let mut replay = Replay {
                        p1_seed: [0; 16],
                        p2_seed: [0; 16],
                        garbage_seed: [0; 16],
                        config: Default::default(),
                        updates: VecDeque::new()
                    };
                    std::mem::swap(&mut replay, &mut self.battle.replay);
                    for _ in 0..180 {
                        replay.updates.push_back(Default::default());
                    }
                    return Some((replay, self.p1_num))
                }
                _ => {}
            }
        }

        None
    }
}

struct GenInfoEvaluator<E> {
    name: IndividualName,
    eval: E
}

impl<E: Evaluator> Evaluator for GenInfoEvaluator<E> {
    fn info(&self) -> Info {
        let mut info = self.eval.info();
        info.insert(1, (format!("{}", self.name), None));
        info
    }

    fn evaluate(&mut self, lock: &LockResult, board: &Board, soft_dropped: bool) -> Evaluation {
        self.eval.evaluate(lock, board, soft_dropped)
    }
}

pub fn playout<E: Evaluator + Clone + Send + 'static>(
    pop: &Population<E>,
    mut matchups: Vec<(usize, usize, (Seed, Seed))>
) -> Vec<usize> {
    let mut victories = vec![0; pop.individuals.len()];
    let mut games = [None, None, None, None, None];

    let current_replay = Arc::new(Mutex::new(None));
    {
        let current_replay = Arc::downgrade(&current_replay);
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            if let Some(current_replay) = current_replay.upgrade() {
                let replay;
                {
                    let mut data = current_replay.lock().unwrap();
                    replay = data.take();
                }
                if let Some(r) = replay {
                    std::fs::File::create("most-recent-replay.json").ok()
                        .and_then(|f| serde_json::to_writer(std::io::BufWriter::new(f), &r).ok());
                }
            }
        });
    }

    const TICK_TIME: std::time::Duration = std::time::Duration::from_millis(2);

    let mut t = std::time::Instant::now();
    loop {
        if std::time::Instant::now() - t >= TICK_TIME {
            let mut not_in_progress = 0;
            for game in &mut games {
                match game {
                    None => match matchups.pop() {
                        Some((p1, p2, (s1, s2))) => *game = Some(Battle::new(
                            pop.individuals[p1].0.clone(), p1, s1, pop.individuals[p1].1,
                            pop.individuals[p2].0.clone(), p2, s2, pop.individuals[p2].1
                        )),
                        None => not_in_progress += 1
                    }
                    Some(battle) => match battle.update() {
                        Some((replay, winner)) => {
                            victories[winner] += 1;
                            *current_replay.lock().unwrap() = Some(replay);
                            *game = None;
                        }
                        None => {}
                    }
                }
            }

            if not_in_progress == 5 {
                break
            }

            t += TICK_TIME;
        } else if std::time::Instant::now() - t > std::time::Duration::from_micros(500) {
            std::thread::sleep(std::time::Duration::from_micros(500))
        }
    }

    victories
}