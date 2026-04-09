## 2025-05-15 - [LogWatcher Optimizations]
**Learning:** In high-frequency log parsing, small overheads like multiple `Regex` initializations for the same pattern, redundant string allocations (`to_string()`), and repeated iterator creation (`split().nth()`) accumulate into measurable bottlenecks. Lazy evaluation for expensive operations (like timestamp parsing) ensures they only run when a relevant event is matched.
**Action:** Consolidate identical regexes, use `max_by_key` for O(N) file searches, and prioritize direct string slicing over iterators in hot parsing paths.
