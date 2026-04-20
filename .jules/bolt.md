## 2025-05-15 - [Log Watcher Optimizations]
**Learning:** Replacing synchronous `std::fs` operations with `tokio::fs` in hot loops (like log polling) prevents blocking the async executor. Using $O(N)$ single-pass search for latest files and early-return heuristics in string parsing significantly reduces CPU and memory overhead for verbose logs.
**Action:** Always prefer async I/O for file watching and implement early-return heuristics for heavy string/regex processing paths.
