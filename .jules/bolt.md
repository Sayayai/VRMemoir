## 2025-05-14 - Log processing optimization
**Learning:** Using a `peekable` iterator for line-by-line processing of a buffer allows handling incomplete lines (without a trailing newline) without needing to collect all lines into a `Vec`. This reduces allocations per read cycle.
**Action:** Prefer `peekable` iterators over `collect::<Vec<_>>()` for streaming or chunked text processing.

## 2025-05-14 - Log parsing heuristics
**Learning:** Adding a simple keyword-based early return in log parsing functions significantly reduces the overhead of more expensive operations (like `contains` with longer strings or regex) for the majority of irrelevant log lines. Lazy timestamp parsing further optimizes the "hot" path.
**Action:** Implement quick heuristic checks and lazy evaluation for common log/event parsing tasks.
