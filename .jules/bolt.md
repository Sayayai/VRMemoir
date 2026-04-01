# Bolt's Performance Journal

## 2026-03-08 - Log Watcher Optimizations

**Learning:**
1. **Directory Scanning:** Using `max_by_key` instead of `sort().first()` reduces complexity from O(N log N) to O(N). In directories with many log files (common for VRChat), this saves unnecessary metadata fetches and comparisons.
2. **Parsing Heuristics:** Most log lines are noise. A simple `contains` check for a few keywords as an early-return guard significantly reduces the CPU time spent on string splitting, regex, and timestamp formatting.
3. **Lazy Evaluation:** Moving expensive string formatting (like timestamp conversion) into a lazy closure ensures it only runs for the <1% of lines that actually represent a relevant event.
4. **Zero-Allocation Iteration:** Using a `peekable` iterator over lines instead of `collect::<Vec<&str>>()` avoids unnecessary allocations and multiple passes over the same memory when processing log buffers.

**Action:**
- Always prefer `max_by_key` for finding a single extreme value in a collection.
- Implement fast-path heuristics for high-frequency processing loops (like log tailing).
- Use lazy evaluation for expensive data preparation steps that might not be needed.
- Avoid `collect()` in hot paths when a single-pass iterator is sufficient.
