use ggez::event::EventHandler;
use ggez::{ Context, GameResult };
use ggez::graphics;
use ggez::timer;
use libtetris::Board;
use battle::{ Game, GameConfig };
use crate::interface::{ Gui, text };
use crate::Resources;
use rand::prelude::*;
use crate::input::InputSource;
use std::collections::VecDeque;

type InputFactory = dyn Fn(Board) -> (Box<dyn InputSource>, String);

pub struct LocalGame<'a> {
    gui: Gui,
    game: Game,
    rng: rand_pcg::Pcg64Mcg,
    p1_input_factory: Box<InputFactory>,
    p1_input: Box<dyn InputSource>,
    p1_info_updates: VecDeque<Option<bot::Info>>,
    state: State,
    resources: &'a mut Resources,
    config: GameConfig,
    time: u32
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
        config: GameConfig
    ) -> Self {
        let mut rng = rand_pcg::Pcg64Mcg::from_seed(thread_rng().gen());
        let game = Game::new(config, &mut rng);
        let (p1_input, p1_name) = p1(game.board.to_compressed());
        LocalGame {
            p1_input,
            p1_input_factory: p1,
            gui: Gui::new(&game, p1_name),
            game, rng, time: 0,
            p1_info_updates: VecDeque::new(),
            state: State::Starting(180),
            resources,
            config
        }
    }
}

impl EventHandler for LocalGame<'_> {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        while timer::check_update_time(ctx, 60) {
            let do_update = match self.state {
                State::GameOver(0) => {
                    self.rng = rand_pcg::Pcg64Mcg::from_seed(thread_rng().gen());
                    self.game = Game::new(self.config, &mut self.rng);

                    let (p1_input, p1_name) = (self.p1_input_factory)(
                        self.game.board.to_compressed()
                    );

                    self.gui = Gui::new(&self.game, p1_name.clone());
                    self.p1_input = p1_input;

                    self.p1_info_updates.clear();

                    self.state = State::Starting(180);
                    self.time = 0;
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
                self.time += 1;
                let p1_controller = self.p1_input.controller(ctx);

                let update = self.game.update(p1_controller, &mut self.rng, &mut thread_rng());

                let p1_info_update = self.p1_input.update(
                    &self.game.board, &update
                );

                self.p1_info_updates.push_back(p1_info_update.clone());

                if let State::Playing = self.state {
                    for event in &update {
                        use battle::Event::*;
                        match event {
                            GameOver => {
                                self.state = State::GameOver(300);
                            }
                            _ => {}
                        }
                    }
                }

                self.gui.update(&update, self.time, p1_info_update, self.resources)?;
            }
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let (scale, center) = crate::interface::setup_graphics(ctx)?;

        if let State::Starting(t) = self.state {
            let txt = text(format!("{}", t / 60 + 1), scale * 4.0, 10.0*scale);
            graphics::queue_text(ctx, &txt, [center-5.0*scale, 9.0*scale], None);
        }

        self.gui.draw(ctx, self.resources, scale, center)?;

        graphics::set_window_title(ctx, &format!("Cold Clear (FPS: {:.0})", ggez::timer::fps(ctx)));

        graphics::present(ctx)
    }
}
