## 2026-03-08 - Log Watcher Optimization Patterns
**Learning:** O(N) `max_by_key` search on file metadata is significantly faster than O(N log N) sorting when only the single latest file is needed. Additionally, deferring expensive operations like timestamp parsing and String allocations until a line is confirmed to be an event of interest reduces CPU and memory pressure in hot loops (log polling).
**Action:** Always prefer one-pass iterators (`max_by_key`, `find`, `peekable`) and lazy evaluation for log-processing logic to minimize overhead.
