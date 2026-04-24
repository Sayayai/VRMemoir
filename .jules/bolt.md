# Bolt's Performance Journal

## 2026-03-08 - Optimized Log Processing and Regex Compilation

**Learning:** Processing large VRChat logs (~60k lines) line-by-line is a hot path where small inefficiencies multiply. `String::replace` and `split().nth(1)` create unnecessary allocations. Deferring expensive operations (like timestamp parsing) until a keyword match occurs significantly reduces CPU and memory overhead on non-event lines.

**Action:** Always use `peekable` iterators for line processing to avoid collecting into vectors. Use `find` and direct slicing with pre-calculated offsets for string extraction. Extract frequently used regexes to static `Lazy` blocks.
