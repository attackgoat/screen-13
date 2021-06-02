# Testing _Screen 13_

Run the following command, as it avoids the use of any physical GPU and instead uses a
"[test](./gfx-backend-test/)" hardware implementation.

In the root _Screen 13_ project directory:

```bash
cargo test --features "blend-modes test-gfx" --no-default-features
```

or

```bash
RUST_BACKTRACE=1 RUST_LOG=trace cargo test --features "blend-modes test-gfx" --no-default-features -- --nocapture dirty_data_compacts
```
