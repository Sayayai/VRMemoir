## 2025-02-14 - Optimized VRChat Log Watcher

**Learning:** Using synchronous file I/O in an async context blocks the executor, impacting overall system responsiveness. (N \log N)$ sorting for finding the latest file is unnecessary when (N)$ traversal is sufficient. Collecting lines into a `Vec` before processing causes redundant allocations.

**Action:** Always prefer `tokio::fs` in async contexts. Use `max_by_key` or manual accumulation for finding extrema in collections. Use `peekable` iterators for line processing to handle incomplete lines without full collection. Implement early returns and lazy evaluation (closures) for hot path parsing.
