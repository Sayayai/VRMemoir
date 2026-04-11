## 2025-05-15 - [Borrow checker pitfall with OsString::to_string_lossy]
**Learning:** Calling `to_string_lossy()` on a temporary `OsString` returned by `DirEntry::file_name()` leads to a "temporary value dropped while borrowed" error because `to_string_lossy()` returns a `Cow<str>` that borrows from the `OsString`.
**Action:** Bind the `OsString` to a variable first to extend its lifetime, or call `.into_owned()` on the result of `to_string_lossy()` if an owned `String` is needed.
