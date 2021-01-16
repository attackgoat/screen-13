# Benchmarking _Screen 13_

Run this command, it avoids the use of any physical GPU and instead uses an
"[empty](https://crates.io/crates/gfx-backend-empty)" device implementation:

```bash
cargo bench --features "no-gfx"
```
