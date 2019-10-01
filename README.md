# Strafe Jump Trainer [![Build Status](https://travis-ci.com/zvxryb/strafe-jump-trainer.svg?branch=master)](https://travis-ci.com/zvxryb/strafe-jump-trainer)

![screenshot](./screenshot.png)

## Running

Run `wasm-pack build --target no-modules` from source root to build a new WASM binary, then change to the `utils` subdirectory and call `cargo run` to serve locally at 127.0.0.1:8080.

## License

This is primarily licensed as GPLv3.  Specific components may be other licenses (i.e. `src/gl_context.rs` is MIT).