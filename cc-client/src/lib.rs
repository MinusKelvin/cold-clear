// #![windows_subsystem = "windows"]

use std::collections::HashSet;
use std::io::prelude::*;

use battle::GameConfig;
use cold_clear::evaluation::Evaluator;
use cold_clear::Book;
use game_util::prelude::*;
use game_util::winit::dpi::{LogicalSize, PhysicalSize};
use game_util::winit::event::{ElementState, VirtualKeyCode, WindowEvent};
use game_util::winit::event_loop::EventLoopProxy;
use game_util::winit::window::{Window, WindowBuilder};
use game_util::{GameloopCommand, LocalExecutor};
use gilrs::{Gamepad, GamepadId, Gilrs};
use serde::de::DeserializeOwned;

mod battle_ui;
mod input;
mod player_draw;
mod realtime;
mod replay;
mod res;

use realtime::RealtimeGame;
use replay::ReplayGame;

struct CCGui {
    log: LogFile,
    gl: Gl,
    psize: PhysicalSize<u32>,
    res: res::Resources,
    el_proxy: EventLoopProxy<Box<dyn State>>,
    executor: LocalExecutor,
    state: Box<dyn State>,
    gilrs: Gilrs,
    keys: HashSet<VirtualKeyCode>,
    p1: Option<GamepadId>,
    p2: Option<GamepadId>,
}

impl game_util::Game for CCGui {
    type UserEvent = Box<dyn State>;

    fn update(&mut self, _: &Window) -> GameloopCommand {
        let gilrs = &self.gilrs;
        let p1 = self.p1.map(|id| gilrs.gamepad(id));
        let p2 = self.p2.map(|id| gilrs.gamepad(id));
        self.state.update(
            &self.el_proxy,
            &self.executor,
            &mut self.log,
            &mut self.res,
            &self.keys,
            p1,
            p2,
        );
        GameloopCommand::Continue
    }

    fn render(&mut self, _: &Window, _: f64, _smooth_delta: f64) {
        while let Some(event) = self.gilrs.next_event() {
            match event.event {
                gilrs::EventType::Connected => {
                    if self.p1.is_none() {
                        self.p1 = Some(event.id);
                    } else if self.p2.is_none() {
                        self.p2 = Some(event.id);
                    }
                }
                gilrs::EventType::Disconnected => {
                    if self.p1 == Some(event.id) {
                        self.p1 = None;
                    } else if self.p2 == Some(event.id) {
                        self.p2 = None;
                    }
                }
                _ => {}
            }
        }

        const TARGET_ASPECT: f64 = 40.0 / 23.0;
        let vp = if (self.psize.width as f64 / self.psize.height as f64) < TARGET_ASPECT {
            PhysicalSize::new(
                self.psize.width,
                (self.psize.width as f64 / TARGET_ASPECT) as u32,
            )
        } else {
            PhysicalSize::new(
                (self.psize.height as f64 * TARGET_ASPECT) as u32,
                self.psize.height,
            )
        };
        self.res.text.dpi = vp.width as f32 / 40.0;

        unsafe {
            self.gl.viewport(
                ((self.psize.width - vp.width) / 2) as i32,
                ((self.psize.height - vp.height) / 2) as i32,
                vp.width as i32,
                vp.height as i32,
            );
            self.gl
                .clear_buffer_f32_slice(glow::COLOR, 0, &mut [0.0f32; 4]);
        }

        self.state.render(&mut self.res);

        self.res.text.render();
    }

    fn event(&mut self, _: &Window, event: WindowEvent) -> GameloopCommand {
        self.state.event(&mut self.res, &event);
        match event {
            WindowEvent::CloseRequested => return GameloopCommand::Exit,
            WindowEvent::Resized(new_size) => {
                self.psize = new_size;
            }
            WindowEvent::KeyboardInput { input, .. } => {
                if let Some(k) = input.virtual_keycode {
                    if input.state == ElementState::Pressed {
                        self.keys.insert(k);
                    } else {
                        self.keys.remove(&k);
                    }
                }
            }
            _ => {}
        }
        GameloopCommand::Continue
    }

    fn user_event(&mut self, _: &Window, new_state: Box<dyn State>) -> GameloopCommand {
        self.state = new_state;
        GameloopCommand::Continue
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    let mut log = LogFile::default();
    let replay_file = std::env::args().skip(1).next();

    game_util::launch(
        WindowBuilder::new()
            .with_title("Cold Clear")
            .with_inner_size(LogicalSize::new(1280.0, 720.0)),
        60.0,
        true,
        |window, gl, el_proxy, executor| {
            unsafe {
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            }
            let psize = window.inner_size();

            let options = match game_util::load("options", true) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    game_util::store("options", &Options::default(), true).ok();
                    Options::default()
                }
                Err(e) => {
                    writeln!(log, "An error occurred while loading options.yaml: {}", e).ok();
                    Options::default()
                }
            };

            let gilrs = Gilrs::new().unwrap_or_else(|e| match e {
                gilrs::Error::NotImplemented(g) => {
                    writeln!(log, "Gamepads are not supported on this platform.").ok();
                    g
                }
                e => {
                    writeln!(log, "Failure initializing gamepad support: {}", e).ok();
                    panic!()
                }
            });
            let mut gamepads = gilrs.gamepads();
            let p1_gamepad = gamepads.next().map(|(id, _)| id);
            let p2_gamepad = gamepads.next().map(|(id, _)| id);

            async move {
                CCGui {
                    log,
                    psize,
                    res: res::Resources::load(&gl, &executor).await,
                    el_proxy,
                    executor,
                    state: match replay_file {
                        Some(f) => Box::new(ReplayGame::new(f)),
                        None => Box::new(RealtimeGame::new(options, 0, 0).await),
                    },
                    p1: p1_gamepad,
                    p2: p2_gamepad,
                    gilrs,
                    keys: HashSet::new(),
                    gl,
                }
            }
        },
    );
}

trait State {
    fn update(
        &mut self,
        el_proxy: &EventLoopProxy<Box<dyn State>>,
        executor: &LocalExecutor,
        log: &mut LogFile,
        res: &mut res::Resources,
        keys: &HashSet<VirtualKeyCode>,
        p1: Option<Gamepad>,
        p2: Option<Gamepad>,
    );
    fn render(&mut self, res: &mut res::Resources);
    fn event(&mut self, _res: &mut res::Resources, _event: &WindowEvent) {}
}

#[derive(Serialize, Deserialize, Clone)]
struct Options {
    p1: PlayerConfig<cold_clear::evaluation::Standard>,
    p2: PlayerConfig<cold_clear::evaluation::Standard>,
}

impl Default for Options {
    fn default() -> Self {
        let mut p2 = PlayerConfig::default();
        p2.is_bot = true;
        Options {
            p1: PlayerConfig::default(),
            p2,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct PlayerConfig<E: Default> {
    controls: input::UserInput,
    game: GameConfig,
    bot_config: BotConfig<E>,
    is_bot: bool,
}

impl<E> PlayerConfig<E>
where
    E: Evaluator + Default + Clone + Serialize + DeserializeOwned + std::fmt::Debug + 'static,
    E::Reward: Serialize + DeserializeOwned,
    E::Value: Serialize + DeserializeOwned,
{
    pub async fn to_player(
        &self,
        board: libtetris::Board,
    ) -> (Box<dyn input::InputSource>, String) {
        use crate::input::BotInput;
        if self.is_bot {
            let mut name = format!("Cold Clear\n{}", self.bot_config.weights.name());
            if self.bot_config.speed_limit != 0 {
                name.push_str(&format!(
                    "\n{:.1}%",
                    100.0 / (self.bot_config.speed_limit + 1) as f32
                ));
            }
            #[cfg(not(target_arch = "wasm32"))]
            let result = (
                Box::new(BotInput::new(
                    cold_clear::Interface::launch(
                        board,
                        self.bot_config.options,
                        self.bot_config.weights.clone(),
                        self.bot_config.book_path.as_ref().and_then(|path| {
                            let mut book_cache = self.bot_config.book_cache.borrow_mut();
                            match &*book_cache {
                                Some(b) => Some(b.clone()),
                                None => {
                                    let book = Book::load(path).ok()?;
                                    let book = std::sync::Arc::new(book);
                                    *book_cache = Some(book.clone());
                                    Some(book)
                                }
                            }
                        }),
                    ),
                    self.bot_config.speed_limit,
                )) as Box<_>,
                name,
            );

            #[cfg(target_arch = "wasm32")]
            let result = (
                Box::new(BotInput::new(
                    cold_clear::Interface::launch(
                        "./worker.js",
                        board,
                        self.bot_config.options,
                        self.bot_config.weights.clone(),
                    )
                    .await,
                    self.bot_config.speed_limit,
                )) as Box<_>,
                name,
            );

            result
        } else {
            (Box::new(self.controls), "Human".to_owned())
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct BotConfig<E> {
    weights: E,
    options: cold_clear::Options,
    speed_limit: u32,
    book_path: Option<String>,
    #[serde(skip)]
    book_cache: std::rc::Rc<std::cell::RefCell<Option<std::sync::Arc<Book>>>>,
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
