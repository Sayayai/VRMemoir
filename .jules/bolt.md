## 2025-03-08 - [LogWatcher Performance Optimization]
**Learning:** In hot paths like log watchers, small overheads compound. 1) $O(N \log N)$ sorting for finding the latest log file is unnecessary when $O(N)$ `max_by_key` suffices. 2) Unconditional string allocations (like `parse_timestamp`) on every line are expensive when most lines are noise. 3) `collect()` into a `Vec` for line iteration causes avoidable heap allocations.
**Action:** Use early return heuristics and defer expensive operations (parsing, allocation) until relevance is confirmed. Prefer direct iteration over `collect()`.
