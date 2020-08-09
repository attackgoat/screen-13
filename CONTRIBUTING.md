# Contributing to Screen 13

Thank you for taking the time to look over this document. You are encouraged to open issues, submit PRs, suggest changes, or anything else you feel might move this project forward.

If you have any questions or would like private coorespondence with the main author, John Wells, please use john@attackgoat.com.

## Ground Rules

This section lists the absolute minimum requirements you must understand and practice in order to be involved with this project. If you fail to uphold the spirit of these rules you will not have any access to make changes within this project and you may be banned entirely.

## Licesnsing Requirements

All contributions, ideas, issues, or other efforts you expend on this project must be providing using the existing MIT or Apache 2.0 license agreements applied to this project. This means that anything you do for this project will be provided to the public without any strings or conditions attached. You must also have the right to provide any code or ideas under these licenses as you will retain no ownership or control after contribution.

## Technical Requirements

All code must:

- Be modern Rust code (currently 2018 edition, latest stable compiler)
- Pass `cargo fmt` and `cargo clippy` (debug and release) with no warnings
- Support required platforms: Linux, Mac, Windows, Web Assembly
- Use only `crates.io`-published crates

### Recommentations

All code should:

- Follow the [guidelines](https://rust-lang.github.io/api-guidelines/)
- Provide useful documentation and comments, including private code
- Expect future platforms: Android, iOS

Most code should:

- Create a new file for `struct`'s with non-trait `impl` blocks
- Group all `use` statements into one nested `use` statement; except for visibility modifiers and conditional compilation attributes
- No star-style `use` statements except in demo code which is intended to be concise
- No `super::super` paths in `use` statements, search by `crate::` for those
- Prefer alphabetical sorting of code items unless there is a well-known non-alpha context
- Use `// TODO` or `// HACK` comments as needed when breaking these guidelines
- Make small `unsafe` blocks as needed, separating the safe and unsafe parts for clarity

### Layout

If you are wondering what logic is used to order and layout the code, you might be interested in this list. These *totally optional* and *completely silly* details should probably be ignored but I am including them for future-me and to give more context. In some cases the code doesn't follow these guidelines and _that's okay_.

Order each file:

- Package-level attributes
- `pub mod` then `pub(modifier) mod` then `mod`
- `pub use` then `pub(modifier) use` then `use`, with the itmes of each group listing attributed `use` statements after the non-attributed one
- `const` then `static` bindings
- `type` aliases
- `fn` implementations
- `enum` and `struct` and `trait` blocks
- `#[cfg(test)]` modules

Special cases:

- `impl` blocks - Always in the same file directly after their `struct` definition, ordered by generics then alphabetically
- macro invocations are placed where the items created by the macro output should otherwise appear

## How do I get started

Download the source code and play around with the examples, see README.md for more.

Any and all contributions are acceptable, including major changes to design or capabilities. If your change is big please ask or open an issue to make sure the rest of the community is on board with your new ideas.

### What if my idea is too radical

If we cannot fit your change into the existing design without breaking API for other users or creating other headaches then we will add your code into a `contrib` directory or separate branch as needed.
