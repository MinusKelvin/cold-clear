use game_util::prelude::*;
use game_util::glutin::{ WindowedContext, PossiblyCurrent };
use game_util::winit::event_loop::EventLoop;
use game_util::winit::window::WindowBuilder;
use game_util::winit::dpi::LogicalSize;

pub type Context = WindowedContext<PossiblyCurrent>;

pub fn create_context(el: &mut EventLoop<()>) -> Result<(Context, Gl), Box<dyn std::error::Error>> {
    game_util::desktop::create_context(
        WindowBuilder::new()
            .with_title("Cold Clear")
            .with_inner_size(LogicalSize::new(1280.0, 720.0)),
        0, true, el
    )
}

pub fn get_options() -> Result<super::Options, Box<dyn std::error::Error>> {
    match std::fs::read_to_string("options.yaml") {
        Ok(options) => Ok(serde_yaml::from_str(&options)?),
        Err(e) => if e.kind() == std::io::ErrorKind::NotFound {
            let ser = serde_yaml::to_string(&super::Options::default())?;
            let mut s = include_str!("options-header").to_owned();
            s.push_str(&ser);
            std::fs::write("options.yaml", &s)?;
            Ok(Default::default())
        } else {
            Err(e.into())
        }
    }
}
