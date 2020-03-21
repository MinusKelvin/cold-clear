use game_util::prelude::*;
use game_util::GameloopCommand;
use game_util::glutin::*;
use game_util::glutin::dpi::LogicalSize;

mod player_draw;
mod battle_ui;
mod res;
mod realtime;
mod input;

use realtime::RealtimeGame;

struct CCGui<'a> {
    context: &'a WindowedContext<PossiblyCurrent>,
    lsize: LogicalSize,
    res: res::Resources,
    state: Box<dyn State>
}

impl game_util::Game for CCGui<'_> {
    fn update(&mut self) -> GameloopCommand {
        self.state.update();
        GameloopCommand::Continue
    }

    fn render(&mut self, _: f64, smooth_delta: f64) {
        let dpi = self.context.window().get_hidpi_factor();
        const TARGET_ASPECT: f64 = 35.0 / 23.0;
        let vp = if self.lsize.width / self.lsize.height < TARGET_ASPECT {
            LogicalSize::new(self.lsize.width, self.lsize.width / TARGET_ASPECT)
        } else {
            LogicalSize::new(self.lsize.height * TARGET_ASPECT, self.lsize.height)
        };
        self.res.text.dpi = (dpi * vp.width / 35.0) as f32;

        unsafe {
            let (rw, rh): (u32, _) = self.lsize.to_physical(dpi).into();
            let (rw, rh) = (rw as i32, rh as i32);
            let (w, h): (u32, _) = vp.to_physical(dpi).into();
            let (w, h) = (w as i32, h as i32);

            gl::Viewport((rw - w) / 2, (rh - h) / 2, w, h);
            gl::ClearBufferfv(gl::COLOR, 0, [0.0f32; 4].as_ptr());
        }

        self.state.render(&mut self.res);

        self.res.text.draw_text(
            "Hello World!",
            1.0, 1.0,
            [0xFF; 4],
            0.8,
            0
        );
        self.res.text.render();

        self.context.window().set_title(
            &format!("Cold Clear (FPS: {:.0})", 1.0/smooth_delta)
        );

        self.context.swap_buffers().unwrap();
    }

    fn event(&mut self, event: WindowEvent, _: WindowId) -> GameloopCommand {
        match event {
            WindowEvent::CloseRequested => return GameloopCommand::Exit,
            WindowEvent::Resized(new_size) => {
                self.lsize = new_size;
                self.context.resize(new_size.to_physical(
                    self.context.window().get_hidpi_factor()
                ));
            }
            _ => {}
        }
        GameloopCommand::Continue
    }
}

fn main() {
    let mut events = EventsLoop::new();

    let (context, lsize) = game_util::create_context(
        WindowBuilder::new()
            .with_title("Cold Clear")
            .with_dimensions((1280.0, 720.0).into()),
        0, true, &mut events
    );

    unsafe {
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }

    let mut game = CCGui {
        context: &context,
        lsize,
        res: res::Resources::load(),
        state: Box::new(RealtimeGame::new(
            Box::new(move |board| (Box::new(input::BotInput::new(cold_clear::Interface::launch(
                board,
                cold_clear::Options::default(),
                cold_clear::evaluation::Standard::default()
            ))), "".to_string())),
            Box::new(move |board| (Box::new(input::BotInput::new(cold_clear::Interface::launch(
                board,
                cold_clear::Options::default(),
                cold_clear::evaluation::Standard::default()
            ))), "".to_string())),
            battle::GameConfig::default(), battle::GameConfig::default()
        ))
    };

    game_util::gameloop(&mut events, &mut game, 60.0, true);
}

trait State {
    fn update(&mut self) -> Option<Box<dyn State>>;
    fn render(&mut self, res: &mut res::Resources);
    fn event(&mut self, event: WindowEvent) -> Option<Box<dyn State>>;
}