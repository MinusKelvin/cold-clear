use ggez::event::EventHandler;
use ggez::{ Context, GameResult };
use ggez::graphics;
use ggez::timer;
use libtetris::{ Battle, Board };
use crate::interface::{ Gui, text };
use crate::Resources;
use rand::prelude::*;
use crate::input::InputSource;

type InputFactory = dyn Fn(Board) -> Box<dyn InputSource>;

pub struct LocalGame<'a> {
    gui: Gui,
    battle: Battle,
    p1_input_factory: Box<InputFactory>,
    p2_input_factory: Box<InputFactory>,
    p1_input: Box<dyn InputSource>,
    p2_input: Box<dyn InputSource>,
    p1_wins: u32,
    p2_wins: u32,
    state: State,
    resources: &'a mut Resources
}

enum State {
    Playing,
    GameOver(u32),
    Starting(u32)
}

impl<'a> LocalGame<'a> {
    pub fn new(resources: &'a mut Resources, p1: Box<InputFactory>, p2: Box<InputFactory>) -> Self {
        let battle = Battle::new(
            Default::default(), thread_rng().gen(), thread_rng().gen(), thread_rng().gen()
        );
        LocalGame {
            p1_input: p1(battle.player_1.board.to_compressed()),
            p2_input: p2(battle.player_2.board.to_compressed()),
            p1_input_factory: p1,
            p2_input_factory: p2,
            gui: Gui::new(&battle),
            battle,
            p1_wins: 0,
            p2_wins: 0,
            state: State::Starting(500),
            resources
        }
    }
}

impl EventHandler for LocalGame<'_> {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        while timer::check_update_time(ctx, 60) {
            let do_update = match self.state {
                State::GameOver(0) => {
                    let t = std::time::Instant::now();
                    serde_json::to_writer(
                        std::io::BufWriter::new(
                            std::fs::File::create("test-replay.json"
                        ).unwrap()),
                        &self.battle.replay
                    ).unwrap();
                    println!("Took {:?} to save replay", t.elapsed());

                    // Don't catch up after pause due to replay saving
                    while timer::check_update_time(ctx, 60) {}

                    self.battle = Battle::new(
                        Default::default(),
                        thread_rng().gen(), thread_rng().gen(), thread_rng().gen()
                    );
                    self.gui = Gui::new(&self.battle);

                    self.p1_input = (self.p1_input_factory)(
                        self.battle.player_1.board.to_compressed()
                    );
                    self.p2_input = (self.p2_input_factory)(
                        self.battle.player_2.board.to_compressed()
                    );

                    self.state = State::Starting(180);
                    false
                }
                State::GameOver(ref mut delay) => {
                    *delay -= 1;
                    true
                }
                State::Starting(0) => {
                    self.state = State::Playing;
                    true
                }
                State::Starting(ref mut delay) => {
                    *delay -= 1;
                    false
                }
                State::Playing => true
            };

            if do_update {
                let p1_controller = self.p1_input.controller(ctx);
                let p2_controller = self.p2_input.controller(ctx);

                let mut update = self.battle.update(p1_controller, p2_controller);

                update.player_1.info = self.p1_input.update(
                    &self.battle.player_1.board, &update.player_1.events
                );
                self.battle.replay.updates.back_mut().unwrap().1 = update.player_1.info.clone();

                update.player_2.info = self.p2_input.update(
                    &self.battle.player_2.board, &update.player_2.events
                );
                self.battle.replay.updates.back_mut().unwrap().3 = update.player_2.info.clone();

                if let State::Playing = self.state {
                    for event in &update.player_1.events {
                        use libtetris::Event::*;
                        match event {
                            GameOver => {
                                self.p2_wins += 1;
                                self.state = State::GameOver(300);
                            }
                            _ => {}
                        }
                    }
                    for event in &update.player_2.events {
                        use libtetris::Event::*;
                        match event {
                            GameOver => {
                                self.p1_wins += 1;
                                self.state = State::GameOver(300);
                            }
                            _ => {}
                        }
                    }
                }

                self.gui.update(update, self.resources)?;
            }
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let (scale, center) = crate::interface::setup_graphics(ctx)?;

        graphics::queue_text(
            ctx,
            &text(format!("{} - {}", self.p1_wins, self.p2_wins), scale*2.0, 6.0*scale),
            [center-3.0*scale, 19.0*scale],
            None
        );

        if let State::Starting(t) = self.state {
            let txt = text(format!("{}", t / 60 + 1), scale * 4.0, 10.0*scale);
            graphics::queue_text(ctx, &txt, [center-14.5*scale, 9.0*scale], None);
            graphics::queue_text(ctx, &txt, [center+4.5*scale, 9.0*scale], None);
        }

        self.gui.draw(ctx, self.resources, scale, center)?;

        graphics::present(ctx)
    }
}
