#![windows_subsystem = "windows"]

use game_util::prelude::*;
use game_util::GameloopCommand;
use game_util::glutin::*;
use game_util::glutin::dpi::LogicalSize;
use gilrs::{ Gilrs, Gamepad, GamepadId };
use battle::GameConfig;
use std::collections::HashSet;
use std::io::prelude::*;
use cold_clear::evaluation::Evaluator;
use cold_clear::Book;

mod player_draw;
mod battle_ui;
mod res;
mod realtime;
mod replay;
mod input;

use realtime::RealtimeGame;
use replay::ReplayGame;

struct CCGui<'a> {
    log: LogFile,
    context: &'a WindowedContext<PossiblyCurrent>,
    lsize: LogicalSize,
    res: res::Resources,
    state: Box<dyn State>,
    gilrs: Gilrs,
    keys: HashSet<VirtualKeyCode>,
    p1: Option<GamepadId>,
    p2: Option<GamepadId>
}

impl game_util::Game for CCGui<'_> {
    fn update(&mut self) -> GameloopCommand {
        let gilrs = &self.gilrs;
        let p1 = self.p1.map(|id| gilrs.gamepad(id));
        let p2 = self.p2.map(|id| gilrs.gamepad(id));
        if let Some(new_state) = self.state.update(
            &mut self.log, &mut self.res, &self.keys, p1, p2
        ) {
            self.state = new_state;
        }
        GameloopCommand::Continue
    }

    fn render(&mut self, _: f64, smooth_delta: f64) {
        while let Some(event) = self.gilrs.next_event() {
            match event.event {
                gilrs::EventType::Connected => if self.p1.is_none() {
                    self.p1 = Some(event.id);
                } else if self.p2.is_none() {
                    self.p2 = Some(event.id);
                }
                gilrs::EventType::Disconnected => if self.p1 == Some(event.id) {
                    self.p1 = None;
                } else if self.p2 == Some(event.id) {
                    self.p2 = None;
                }
                _ => {}
            }
        }

        let dpi = self.context.window().get_hidpi_factor();
        const TARGET_ASPECT: f64 = 40.0 / 23.0;
        let vp = if self.lsize.width / self.lsize.height < TARGET_ASPECT {
            LogicalSize::new(self.lsize.width, self.lsize.width / TARGET_ASPECT)
        } else {
            LogicalSize::new(self.lsize.height * TARGET_ASPECT, self.lsize.height)
        };
        self.res.text.dpi = (dpi * vp.width / 40.0) as f32;

        unsafe {
            let (rw, rh): (u32, _) = self.lsize.to_physical(dpi).into();
            let (rw, rh) = (rw as i32, rh as i32);
            let (w, h): (u32, _) = vp.to_physical(dpi).into();
            let (w, h) = (w as i32, h as i32);

            gl::Viewport((rw - w) / 2, (rh - h) / 2, w, h);
            gl::ClearBufferfv(gl::COLOR, 0, [0.0f32; 4].as_ptr());
        }

        self.state.render(&mut self.res);

        self.res.text.render();

        self.context.window().set_title(
            &format!("Cold Clear (FPS: {:.0})", 1.0/smooth_delta)
        );

        self.context.swap_buffers().unwrap();
    }

    fn event(&mut self, event: WindowEvent, _: WindowId) -> GameloopCommand {
        if let Some(new_state) = self.state.event(&mut self.res, &event) {
            self.state = new_state;
        }
        match event {
            WindowEvent::CloseRequested => return GameloopCommand::Exit,
            WindowEvent::Resized(new_size) => {
                self.lsize = new_size;
                self.context.resize(new_size.to_physical(
                    self.context.window().get_hidpi_factor()
                ));
            }
            WindowEvent::KeyboardInput { input, .. } => if let Some(k) = input.virtual_keycode {
                if input.state == ElementState::Pressed {
                    self.keys.insert(k);
                } else {
                    self.keys.remove(&k);
                }
            }
            _ => {}
        }
        GameloopCommand::Continue
    }
}

fn main() {
    let mut log = LogFile::default();
    let replay_file = std::env::args().skip(1).next();

    let mut events = EventsLoop::new();

    let (context, lsize) = game_util::create_context(
        WindowBuilder::new()
            .with_title("Cold Clear")
            .with_dimensions((1280.0, 720.0).into()),
        0, true, &mut events
    ).unwrap_or_else(|e| {
        writeln!(
            log, "Failure initializing OpenGL context. Does your computer suport OpenGL 3.3?"
        ).ok();
        writeln!(log, "{}", e).ok();
        panic!()
    });

    unsafe {
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }

    let Options { p1, p2 } = read_options().unwrap_or_else(|e| {
        writeln!(log, "An error occured while loading options.yaml: {}", e).ok();
        Options::default()
    });
    let p1_game_config = p1.game;
    let p2_game_config = p2.game;

    let gilrs = Gilrs::new().unwrap_or_else(|e| match e {
        gilrs::Error::NotImplemented(g) => {
            writeln!(log, "Gamepads are not supported on this platform.").ok();
            g
        },
        e => {
            writeln!(log, "Failure initializing gamepad support: {}", e).ok();
            panic!()
        }
    });
    let mut gamepads = gilrs.gamepads();

    let mut game = CCGui {
        log,
        context: &context,
        lsize,
        res: res::Resources::load(),
        state: match replay_file {
            Some(f) => Box::new(ReplayGame::new(f)),
            None => Box::new(RealtimeGame::new(
                Box::new(move |board| p1.to_player(board)),
                Box::new(move |board| p2.to_player(board)),
                p1_game_config, p2_game_config
            ))
        },
        p1: gamepads.next().map(|(id, _)| id),
        p2: gamepads.next().map(|(id, _)| id),
        gilrs,
        keys: HashSet::new()
    };

    game_util::gameloop(&mut events, &mut game, 60.0, true);
}

trait State {
    fn update(
        &mut self,
        log: &mut LogFile,
        res: &mut res::Resources,
        keys: &HashSet<VirtualKeyCode>,
        p1: Option<Gamepad>,
        p2: Option<Gamepad>
    ) -> Option<Box<dyn State>>;
    fn render(&mut self, res: &mut res::Resources);
    fn event(
        &mut self, _res: &mut res::Resources, _event: &WindowEvent
    ) -> Option<Box<dyn State>> { None }
}

fn read_options() -> Result<Options, Box<dyn std::error::Error>> {
    match std::fs::read_to_string("options.yaml") {
        Ok(options) => Ok(serde_yaml::from_str(&options)?),
        Err(e) => if e.kind() == std::io::ErrorKind::NotFound {
            let ser = serde_yaml::to_string(&Options::default())?;
            let mut s = include_str!("options-header").to_owned();
            s.push_str(&ser);
            std::fs::write("options.yaml", &s)?;
            Ok(Options::default())
        } else {
            Err(e.into())
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Options {
    p1: PlayerConfig<cold_clear::evaluation::Standard>,
    p2: PlayerConfig<cold_clear::evaluation::Standard>
}

impl Default for Options {
    fn default() -> Self {
        let mut p2 = PlayerConfig::default();
        p2.is_bot = true;
        Options {
            p1: PlayerConfig::default(),
            p2
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
struct PlayerConfig<E: Default> {
    controls: input::UserInput,
    game: GameConfig,
    bot_config: BotConfig<E>,
    is_bot: bool,
}

impl<E: Evaluator + Default + Clone + 'static> PlayerConfig<E> {
    pub fn to_player(&self, board: libtetris::Board) -> (Box<dyn input::InputSource>, String) {
        use crate::input::BotInput;
        if self.is_bot {
            let mut name = format!("Cold Clear\n{}", self.bot_config.weights.name());
            if self.bot_config.speed_limit != 0 {
                name.push_str(
                    &format!("\n{:.1}%", 100.0 / (self.bot_config.speed_limit + 1) as f32)
                );
            }
            (Box::new(BotInput::new(cold_clear::Interface::launch(
                board,
                self.bot_config.options,
                self.bot_config.weights.clone(),
                self.bot_config.book_path.as_ref().and_then(|path| {
                    let mut book_cache = self.bot_config.book_cache.borrow_mut();
                    match &*book_cache {
                        Some(b) => Some(b.clone()),
                        None => {
                            let book = Book::load_from(std::fs::File::open(path).ok()?).ok()?;
                            let book = std::sync::Arc::new(book);
                            *book_cache = Some(book.clone());
                            Some(book)
                        }
                    }
                })
            ), self.bot_config.speed_limit)), name)
        } else {
            (Box::new(self.controls), "Human".to_owned())
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
struct BotConfig<E> {
    weights: E,
    options: cold_clear::Options,
    speed_limit: u32,
    book_path: Option<String>,
    #[serde(skip)]
    book_cache: std::cell::RefCell<Option<std::sync::Arc<Book>>>
}

#[derive(Default)]
struct LogFile(Vec<u8>);

impl Write for LogFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl Drop for LogFile {
    fn drop(&mut self) {
        if !self.0.is_empty() {
            std::fs::write("error.log", &self.0).ok();
        } else {
            std::fs::remove_file("error.log").ok();
        }
    }
}
