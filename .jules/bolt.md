# Bolt's Performance Journal

## 2026-03-08 - String processing in hot paths
**Learning:** Using `split().nth(1)` or `replace()`/`replacen()` on every log line introduces significant allocation overhead and redundant string scanning.
**Action:** Use `find()` to locate anchors and direct slicing to extract data. Pre-allocate `String` with `with_capacity()` and use manual byte iteration for format conversions to minimize heap churn.

## 2026-03-08 - Lazy Evaluation of metadata
**Learning:** Parsing metadata like timestamps for every log line is wasteful when only ~1% of lines are relevant events.
**Action:** Defer expensive parsing operations (like date formatting) until after a line has been identified as a relevant event.

## 2026-03-08 - Memory-efficient line processing
**Learning:** `content.lines().collect::<Vec<_>>()` allocates a vector to hold all line references, which can be large for big log chunks.
**Action:** Use `content.lines().peekable()` (or just the iterator) to process lines one-by-one without collecting them into a collection.
