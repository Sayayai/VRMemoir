## 2025-03-24 - Efficient Log Parsing in Rust

**Learning:** When parsing large log files, avoiding allocations in the "hot path" is critical. `content.lines().collect::<Vec<&str>>()` allocates a new vector for every read cycle. Replacing it with a `peekable` iterator allows for $O(1)$ space (plus the line buffers) to identify the incomplete last line. Additionally, lazy evaluation of expensive operations like timestamp string replacement ensures they only run when a relevant log event is actually found.

**Action:** Use iterators instead of collecting into collections when processing streams or large buffers. Prefer `find()` and slicing over `split()` for simple delimiter-based extraction to avoid iterator overhead and potential allocations.
