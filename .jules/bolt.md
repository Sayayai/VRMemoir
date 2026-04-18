## 2026-03-08 - Don't assume [Behaviour] tags for all VRChat log events
**Learning:** Some critical log events like world transitions ("Entering Room") and instance joins ("Joining wrld_") can appear in VRChat logs without the "[Behaviour]" prefix, depending on the log source or version. Using a restrictive early return filter based on tags can cause functional regressions.
**Action:** Use broader keyword matching (e.g. searching for the event name directly) and defer expensive operations like timestamp parsing until after a match is confirmed to maintain both performance and correctness.
