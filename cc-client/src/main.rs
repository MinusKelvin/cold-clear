fn main() {
    if let Some(path) = std::env::var_os("CARGO_MANIFEST_DIR") {
        std::env::set_current_dir(path).ok();
    }

    cc_client::main();
}