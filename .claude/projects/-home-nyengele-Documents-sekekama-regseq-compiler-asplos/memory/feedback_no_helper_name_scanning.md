---
name: No helper name scanning in backend
description: Backend must not pattern-match authored helper names to derive Linux callback mappings
type: feedback
---

Do not scan or pattern-match authored helper function names in the backend to derive Linux callback mappings (e.g., scanning for "enable_tx", "disable_tx" patterns).

**Why:** This is backend reconstruction from authored names, not explicit lowered semantics. It's especially fragile for callbacks like `stop_rx` where no matching authored helper may exist. The backend should only use explicit plan-driven data or honest stubs.

**How to apply:** When a Linux callback has no direct portable `FunctionRole` match (e.g., `start_tx`, `stop_tx`, `stop_rx`), use `Stub` with an honest TODO comment instead of guessing from helper names. Proper realization requires extending the lowering layer to provide explicit plan data for these callbacks.
