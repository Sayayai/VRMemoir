## 2025-05-14 - Optimize Log Parsing Hot Path
**Learning:** VRChat log files can grow very large, and performing timestamp parsing and regex matching on every line is a major bottleneck. Early return heuristics using simple string contains check is extremely effective for skipping irrelevant data.
**Action:** Always implement early return heuristics in file-parsing hot paths before executing complex logic or regex.

## 2025-05-14 - Memory Efficiency in Log Watchers
**Learning:** Using `lines().collect::<Vec<_>>()` on large content buffers causes unnecessary heap allocations. A `peekable` iterator allows processing lines one by one while still handling incomplete trailing lines correctly.
**Action:** Prefer stream-like or iterator-based line processing over collecting into intermediate collections.
