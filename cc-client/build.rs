use std::env;
use std::path::Path;

fn main() {
    build_utils::gen_sprites(
        "sprites",
        "res/generated",
        Path::new(&env::var("OUT_DIR").unwrap()),
        2048,
    );
}
