## 2026-03-23 - Log parsing optimization via early returns and lazy evaluation

**Learning:** VRChat logs generate many lines that are irrelevant to our tracking. Processing every line with timestamp parsing and multiple regex/contains checks is wasteful. Using a simple keyword-based early return and deferring expensive operations (like timestamp parsing) until an event is confirmed significantly reduces CPU overhead in the hot path.

**Action:** Always implement a simple, low-cost heuristic (e.g., `line.contains("keyword")`) before performing complex parsing or regex matching on log streams. Defer string manipulations and timestamp conversions using lazy evaluation (closures) until you are certain the data is needed.
