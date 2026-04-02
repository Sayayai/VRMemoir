## 2026-03-08 - Optimized Log Polling and Parsing
**Learning:** Collecting all lines from a log buffer into a `Vec<&str>` before processing is inefficient for large reads and introduces unnecessary allocations. Using a `peekable` iterator allows for O(1) space (relative to the number of lines) and better handling of trailing partial lines. Additionally, lazy evaluation of timestamps using closures avoids expensive string operations on irrelevant log lines.
**Action:** Use `peekable()` for stream/buffer line processing and closures for lazy field parsing in hot loops.
