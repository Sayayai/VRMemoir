## 2025-03-24 - Efficient Log Parsing in Rust
**Learning:** Collecting all lines from a log buffer into a `Vec<&str>` before processing introduces unnecessary heap allocations and O(N) overhead. Using a `peekable` iterator allows for single-pass processing while still handling incomplete trailing lines correctly. Additionally, early-return heuristics based on simple keyword checks (e.g., `line.contains("[Behaviour]")`) significantly reduce the cost of parsing noisy logs by avoiding regex and complex string logic for irrelevant lines.

**Action:** Prefer streaming/iterator-based line processing over `collect()` for large text buffers. Implement "fast-path" keyword checks before descending into expensive parsing logic.
