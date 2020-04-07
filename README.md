# Pollster

Pollster is an incredibly minimal async executor for Rust that lets you block a thread on the result of a future.

[![Cargo](https://img.shields.io/crates/v/pollster.svg)](
https://crates.io/crates/pollster)
[![Documentation](https://docs.rs/pollster/badge.svg)](
https://docs.rs/pollster)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://github.com/zesterer/pollster)

```rust
let result = pollster::block_on(my_future);
```

That's it. That's all it does. Nothing more. No dependencies, no complexity. No need to drag in 50-odd dependencies to evaluate a future.
