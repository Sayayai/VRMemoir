
## 2026-03-22 - Optimize log watcher performance
**Learning:** O(N log N) sorting for finding the latest log file is inefficient in directories with many files. Eagerly parsing timestamps for every line in a noisy log file is a significant bottleneck. Iterator-based line processing with a `peekable()` iterator avoids unnecessary `Vec` allocations and efficiently handles incomplete lines.
**Action:** Use `max_by_key` for selection in log directories. Implement early return heuristics for log parsing to skip irrelevant lines. Defer expensive operations (like timestamp parsing) until a relevant line is identified.
