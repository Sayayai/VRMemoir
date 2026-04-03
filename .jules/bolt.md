## 2025-05-14 - Optimized Log Processing with Peekable Iterators
**Learning:** Collecting log lines into a `Vec<&str>` before processing introduces unnecessary allocations and peak memory usage, especially for large log files. Using a `peekable` iterator allows for single-pass processing while still handling the "incomplete last line" edge case without extra collection.
**Action:** Prefer `peekable` iterators over `.collect::<Vec<_>>()` when processing stream-like data (logs, network packets) where the final element requires special handling.
