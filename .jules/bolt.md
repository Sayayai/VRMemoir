# Bolt's Performance Journal ⚡

## 2026-03-08 - Initialized log watcher optimization task
**Learning:** Initial exploration of `src/watcher.rs` revealed $O(N \log N)$ sorting for log files and $O(N)$ allocations for line processing in the hot path.
**Action:** Replace `sort_by` with `max_by_key` and use lazy evaluation/iterators to reduce overhead.
