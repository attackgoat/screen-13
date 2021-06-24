# WebAssembly Example

Demonstrates compiling and packaging a _Screen 13_ program for the web.

## Prerequisites

- _Screen 13_: `cargo install screen-13`

## Bake

Bake the source assets into a `.pak` file:

```bash
screen-13 res/wasm.toml
```

## Build

On unix-based systems you may run the [build](./build) script which does the following:

Compile the source code into a `.wasm` file:

```bash
cargo build --lib --target wasm32-unknown-unknown
```

Generate some web glue:

```bash
wasm-bindgen target/wasm32-unknown-unknown/debug/wasm.wasm --out-dir target/wasm32-unknown-unknown/debug --target no-modules
```

## Run

Start a _`Hyper`_ web server, serving up an [example page](http://localhost:8080/):

```bash
cargo run --bin wasm
```
