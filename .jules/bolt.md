## 2026-03-08 - Optimized Log Watcher Performance
**Learning:** Log parsing in `src/watcher.rs` was a significant hot path with redundant string scans and heap allocations. Deferring expensive operations like timestamp parsing and using direct slicing instead of `split()` provides measurable speedups.
**Action:** Always defer expensive formatting/parsing until a keyword match is found. Use `find()` and direct slicing for marker identification in high-frequency text processing.
