# Contributing to Screen 13

Thank you for taking the time to look over this document. You are encouraged to open issues, submit PRs, suggest changes, or anything else you feel might move this project forward.

If you have any questions or would like private coorespondence with the main author, John Wells, please use john@attackgoat.com.

## Ground Rules

This section lists the absolute minimum requirements you must understand and practice in order to be involved with this project. If you fail to uphold the spirit of these rules you will not have any access to make changes within this project and you may be banned entirely.

### Licesnsing Requirements

All contributions, ideas, issues, or other efforts you expend on this project must be providing using the existing MIT or Apache 2.0 license agreements applied to this project. This means that anything you do for this project will be provided to the public without any strings or conditions attached. You must also have the right to provide any code or ideas under these licenses as you will retain no ownership or control after contribution.

### Technical Requirements

All code must:

- Be modern Rust code (currently 2018 edition, latest stable compiler)
- Pass `cargo fmt` and `cargo clippy` (debug and release) with no warnings
- Support required platforms: Linux, Mac, Windows, Web Assembly

All code should:

- Use only `crates.io`-published crates
- Provide useful documentation
- Expect future platforms: Android, iOS
- Follow the [guidelines](https://rust-lang.github.io/api-guidelines/)

Most code should:

- Create a new file or module for `struct`'s with non-trait `impl` blocks
- Group all `use` statements into one nested `use` statement; where possible
- No star-style `use` statements except in demo code which is intended to be concise 
- Prefer alphabetical sorting of code items unless there is a well-known non-alpha context
- Use `// TODO` or `// HACK` comments as needed when breaking these guidelines

### Human Requirements

All contributors must:

- Be courteous and respectful to all humans; including outside of this project
- Provide and respond to feedback in a friendly and reasonable manner

## How do I get started?

Download the source code and play around with the examples, see README.md for more.

Any and all contributions are acceptable, including major changes to design or capabilities. If your change is big please ask or open an issue to make sure the rest of the community is on board with your new ideas.

### What if my idea is too radical?

If we cannot fit your change into the existing design without breaking API for other users or creating other headaches then we will add your code into a `contrib` directory or separate branch as needed.
