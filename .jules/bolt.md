## 2026-03-08 - Optimized Log Watching and Parsing
**Learning:** O(N log N) sorting for finding the latest file in a directory is an anti-pattern when O(N) `max_by_key` is available. Furthermore, log parsing can be significantly sped up by using simple `contains` checks to bail out before doing expensive regex or timestamp parsing.
**Action:** Always prefer `max_by_key` for finding single extremums in iterators. Implement early-return heuristics in hot-path string parsers.
