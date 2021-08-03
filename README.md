# Pollster

Pollster is an incredibly minimal async executor for Rust that lets you block a thread on the result of a future.

[![Cargo](https://img.shields.io/crates/v/pollster.svg)](
https://crates.io/crates/pollster)
[![Documentation](https://docs.rs/pollster/badge.svg)](
https://docs.rs/pollster)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://github.com/zesterer/pollster)
![actions-badge](https://github.com/zesterer/pollster/workflows/Rust/badge.svg?branch=master)

```rust
use pollster::FutureExt as _;

let my_fut = async {};

let result = my_fut.block_on();
```

That's it. That's all it does. Nothing more. No dependencies, no complexity. No need to pull in 50 crates to evaluate a
future.

Note that `pollster` will not work for *arbitrary* futures because some require a specific runtime or reactor. See
[here](https://rust-lang.github.io/async-book/08_ecosystem/00_chapter.html#determining-ecosystem-compatibility) for more
information about when and where `pollster` may be used. However, if you're already pulling in the required dependencies
to create such a future in the first place, it's likely that you already have a version of `block_on` in your dependency
tree that's designed to poll your future, so use that instead.
