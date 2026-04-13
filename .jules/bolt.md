## 2026-04-13 - [Async I/O and Log Parsing Optimization]
**Learning:** In a high-frequency polling loop like log watching, synchronous I/O can block the async runtime, and heavy string operations (like `split`) and redundant regex compilation add significant overhead. Using `tokio::fs`, `peekable` iterators for line processing, and lazy evaluation for timestamps provides a measurable performance boost.
**Action:** Always prefer `tokio::fs` for I/O in async contexts. Use `find()` and slicing instead of `split()` for simple keyword extraction. Consolidate regexes into static `Lazy` instances.
