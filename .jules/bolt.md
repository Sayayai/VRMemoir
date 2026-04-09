# Bolt's Performance Journal

## 2026-03-08 - [Efficient Line Processing in Rust]
**Learning:** Using `lines().collect::<Vec<_>>()` on a large log buffer causes an unnecessary heap allocation for the entire list of line references. Replacing this with a `peekable` iterator allows for O(1) space (excluding the buffer itself) while still enabling logic that needs to identify the final line (e.g., for incomplete line handling).
**Action:** Always prefer lazy iterators or `peekable` iterators for line-by-line processing in hot paths like log watchers.

## 2026-03-08 - [Temporary OsString and to_string_lossy()]
**Learning:** Calling `e.file_name().to_string_lossy()` in a chain like `.filter(|e| e.file_name().to_string_lossy().starts_with(...))` can lead to "temporary value dropped while borrowed" errors because `file_name()` returns an `OsString` (owned), and `to_string_lossy()` returns a `Cow<str>` which might borrow from that temporary.
**Action:** Bind the `OsString` to a local variable first to extend its lifetime when performing string operations in closures.
