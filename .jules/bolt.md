## 2025-05-15 - Async I/O and Log Parsing Optimization
**Learning:** Synchronous file system operations in a Tokio runtime can block the executor, leading to latency. Optimizing hot paths like log parsing with early returns and direct slicing significantly reduces CPU and memory overhead compared to using iterators like `split().nth()`.
**Action:** Always prefer `tokio::fs` in async contexts and use string slicing instead of collecting or multi-pass iteration for high-frequency parsing tasks.
