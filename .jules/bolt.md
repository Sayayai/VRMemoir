## 2026-03-08 - Optimized Log Parsing with Lazy Evaluation and Iterators
**Learning:** In high-frequency log polling, string allocations are the primary bottleneck. Pre-allocating strings for timestamps and using `peekable` iterators to avoid collecting lines into a `Vec` significantly reduces heap churn. Deferring expensive operations (like timestamp parsing) until a match is confirmed prevents thousands of wasted cycles.
**Action:** Always use iterators for line-by-line processing of external files and defer string manipulation until necessary.
