# Cold Clear

Modern Tetris Versus bot.

You can play against it [in your browser](https://minuskelvin.net/cold-clear)
or [on your desktop](https://github.com/MinusKelvin/cold-clear/releases).

## Usage

### As a Rust library

```toml
# Cargo.toml

[dependencies]
cold-clear = { git = "https://github.com/MinusKelvin/cold-clear" }
```

### As a C library

Clone the repository and run `cargo build --release -p c-api` to build. You
can find both static and shared libraries in `target/release`, and the API is
described in [`c-api/coldclear.h`](c-api/coldclear.h).

### Running the desktop client from source

Clone the repository and run `cargo run --release`.

## License

Cold Clear is licensed under [MPLv2](LICENSE).
