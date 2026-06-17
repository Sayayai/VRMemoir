# Bolt's Journal - Performance Learnings

## 2026-03-08 - Optimized Log Watcher and Parser
**Learning:** For chatty log files, even simple operations like timestamp parsing or regex matching can become a bottleneck when multiplied by thousands of lines per second. Early return heuristics using simple keyword checks (`line.contains("[Behaviour]")`) significantly reduce wasted CPU cycles. Additionally, O(N log N) sorting for finding the latest file is wasteful when O(N) `max_by_key` is available, especially as the number of log files grows. Finally, using peekable iterators for line processing avoids unnecessary `Vec` allocations in high-frequency polling loops.
**Action:** Always implement early-return heuristics in log parsers and prefer O(N) operations and zero-allocation iterators in hot paths.
