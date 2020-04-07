# Pollster

Pollster is an incredibly minimal async executor for Rust that lets you block a thread on the result of a future.

```rust
let result = pollster::block_on(my_future);
```

That's it. That's all it does. Nothing more. No dependencies, no complexity. No need to drag in 50-odd dependencies to evaluate a future.
