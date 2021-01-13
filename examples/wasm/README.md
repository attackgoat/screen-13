# WebAssembly Example

Demonstrates compiling and packaging a _Screen 13_ program for the web.

## Prerequisites

- TODO

## Build

Compile into a `.wasm` file:

```bash
cargo +nightly build --lib --target wasm32-unknown-unknown
```

Generate some web glue:

```bash
wasm-bindgen target/wasm32-unknown-unknown/debug/wasm.wasm --out-dir target/wasm32-unknown-unknown/debug --target no-modules
```

## Run

Start a _`Hyper`_ web server, serving up an [example page](http://localhost/):

```bash
cargo run --bin wasm
```
