## 2025-05-15 - [LogWatcher Parsing Optimization]
**Learning:** Using `peekable` iterators for line-by-line log processing allows identifying the last (potentially incomplete) line of a buffer without collecting all lines into a `Vec`, significantly reducing heap allocations in high-frequency I/O paths. Additionally, replacing `sort_by` with `max_by_key` for finding the latest log file reduces complexity from O(N log N) to O(N).
**Action:** Always prefer `peekable` iterators for stream/line processing and `max_by_key`/`min_by_key` for single-element selection from collections.
