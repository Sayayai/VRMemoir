## 2026-03-08 - Optimized VRChat Log Parsing

**Learning:** VRChat log parsing is a high-frequency operation where small inefficiencies in string processing and allocation add up. Lazy parsing of timestamps and avoiding intermediate collections (like `Vec<&str>` for lines) significantly reduces overhead. Single-pass file selection with `max_by_key` is always preferable over sorting for finding the latest file.

**Action:** Always defer expensive formatting/parsing until a match is confirmed. Use `peekable` iterators for line-by-line processing of large buffers to avoid heap allocations. Prioritize `find` and slicing over `split` when looking for specific markers.
