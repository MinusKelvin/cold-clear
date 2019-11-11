use ggez::event::EventHandler;
use ggez::{ Context, GameResult };
use ggez::graphics;
use ggez::timer;
use libtetris::Board;
use battle::{ Battle, GameConfig };
use crate::interface::{ Gui, text };
use crate::Resources;
use rand::prelude::*;
use crate::input::InputSource;
use crate::replay::InfoReplay;
use std::collections::VecDeque;

type InputFactory = dyn Fn(Board) -> (Box<dyn InputSource>, String);

pub struct LocalGame<'a> {
    gui: Gui,
    battle: Battle,
    p1_input_factory: Box<InputFactory>,
    p2_input_factory: Box<InputFactory>,
    p1_input: Box<dyn InputSource>,
    p2_input: Box<dyn InputSource>,
    p1_wins: u32,
    p2_wins: u32,
    p1_info_updates: VecDeque<Option<bot::Info>>,
    p2_info_updates: VecDeque<Option<bot::Info>>,
    state: State,
    resources: &'a mut Resources,
    config: GameConfig
}

enum State {
    Playing,
    GameOver(u32),
    Starting(u32)
}

impl<'a> LocalGame<'a> {
    pub fn new(
        resources: &'a mut Resources,
        p1: Box<InputFactory>,
        p2: Box<InputFactory>,
        config: GameConfig
    ) -> Self {
        let mut battle = Battle::new(
            config, thread_rng().gen(), thread_rng().gen(), thread_rng().gen()
        );
        let (p1_input, p1_name) = p1(battle.player_1.board.to_compressed());
        let (p2_input, p2_name) = p2(battle.player_2.board.to_compressed());
        battle.replay.p1_name = p1_name.clone();
        battle.replay.p2_name = p2_name.clone();
        LocalGame {
            p1_input, p2_input,
            p1_input_factory: p1,
            p2_input_factory: p2,
            gui: Gui::new(&battle, p1_name, p2_name),
            battle,
            p1_wins: 0,
            p2_wins: 0,
            p1_info_updates: VecDeque::new(),
            p2_info_updates: VecDeque::new(),
            state: State::Starting(180),
            resources,
            config
        }
    }
}

impl EventHandler for LocalGame<'_> {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        let mut count = 0;
        while timer::check_update_time(ctx, 60) {
            count += 1;
            if count == 3 {
                while timer::check_update_time(ctx, 60) {}
            }
            let do_update = match self.state {
                State::GameOver(0) => {
                    serde_json::to_writer(
                        std::io::BufWriter::new(
                            std::fs::File::create("replay.json"
                        ).unwrap()),
                        &InfoReplay {
                            replay: self.battle.replay.clone(),
                            p1_info_updates: self.p1_info_updates.clone(),
                            p2_info_updates: self.p2_info_updates.clone()
                        }
                    ).unwrap();

                    // Don't catch up after pause due to replay saving
                    while timer::check_update_time(ctx, 60) {}

                    self.battle = Battle::new(
                        self.config,
                        thread_rng().gen(), thread_rng().gen(), thread_rng().gen()
                    );

                    let (p1_input, p1_name) = (self.p1_input_factory)(
                        self.battle.player_1.board.to_compressed()
                    );
                    let (p2_input, p2_name) = (self.p2_input_factory)(
                        self.battle.player_2.board.to_compressed()
                    );

                    self.gui = Gui::new(&self.battle, p1_name.clone(), p2_name.clone());
                    self.p1_input = p1_input;
                    self.p2_input = p2_input;
                    self.battle.replay.p1_name = p1_name;
                    self.battle.replay.p2_name = p2_name;

                    self.p1_info_updates.clear();
                    self.p2_info_updates.clear();

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

                let update = self.battle.update(p1_controller, p2_controller);

                let p1_info_update = self.p1_input.update(
                    &self.battle.player_1.board, &update.player_1.events
                );
                let p2_info_update = self.p2_input.update(
                    &self.battle.player_2.board, &update.player_2.events
                );

                self.p1_info_updates.push_back(p1_info_update.clone());
                self.p2_info_updates.push_back(p2_info_update.clone());

                if let State::Playing = self.state {
                    for event in &update.player_1.events {
                        use battle::Event::*;
                        match event {
                            GameOver => {
                                self.p2_wins += 1;
                                self.state = State::GameOver(300);
                            }
                            _ => {}
                        }
                    }
                    for event in &update.player_2.events {
                        use battle::Event::*;
                        match event {
                            GameOver => {
                                self.p1_wins += 1;
                                self.state = State::GameOver(300);
                            }
                            _ => {}
                        }
                    }
                }

                self.gui.update(update, p1_info_update, p2_info_update, self.resources)?;
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

        graphics::set_window_title(ctx, &format!("Cold Clear (FPS: {:.0})", ggez::timer::fps(ctx)));

        graphics::present(ctx)
    }
}
