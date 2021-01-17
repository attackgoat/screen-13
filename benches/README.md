# Benchmarking _Screen 13_

Run the following command, as it avoids the use of any physical GPU and instead uses a
"[mock](./gfx-mock/)" hardware implementation.

In the root _Screen 13_ project directory:

```bash
cargo bench --features "mock-gfx" --no-default-features
```
