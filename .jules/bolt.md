## 2026-03-08 - [Log Watcher Performance Anti-patterns]
**Learning:** In long-running monitoring services like `vrmemoir`, naive log processing (e.g., sorting all historical log files or running regex on every irrelevant line) quickly becomes a CPU and memory bottleneck as the number of log files and entries grows.
**Action:** Always prefer O(N) search for the latest file and implement aggressive early-return keyword filters to bypass expensive parsing logic (regex, timestamp formatting) for the ~90% of log noise that doesn't trigger events.
