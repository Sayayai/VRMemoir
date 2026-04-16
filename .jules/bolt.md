## 2026-03-08 - Optimized Log Processing Pipeline
**Learning:** Using `peekable` iterators instead of collecting lines into a `Vec` significantly reduces allocations during high-frequency log polling. Similarly, (N)$ searches for files are much more efficient than sorting when only the latest file is needed.
**Action:** Always prefer lazy iteration and (N)$ searches over collecting and sorting in performance-critical paths.
