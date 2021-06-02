# Benchmarking Screen 13

In the root _Screen 13_ project directory:

```bash
cargo bench --features "mock-gfx" --no-default-features
```

**_NOTE:_** Benchmarking uses a [mock GPU](./gfx-backend-mock/) in order to avoid the use of any
physical hardware.
