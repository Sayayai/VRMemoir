## 2025-03-26 - Optimized Log Parsing Path

**Learning:** VRChat logs generate high volumes of data, making `parse_line` a hot path. Unconditional timestamp parsing and iterator-based string splitting (`split().nth(1)`) were significant overheads for irrelevant lines. Also, collecting all lines into a `Vec` during polling created unnecessary $O(N)$ allocations.

**Action:** Implement early return heuristics for keyword filtering, use lazy evaluation (closures) for expensive parsing operations, and utilize `peekable` iterators to process streams of data without intermediate collection. Replace sorting with `max_by_key` for $O(N)$ searches in file metadata.
