---
name: No silent degradation in supported-slice hierarchy
description: Unresolvable required facts (self types, trait paths) must be hard errors, not silent Named fallbacks
type: feedback
---

Unresolved owning types or required cross-link targets in hierarchy construction must produce hard errors, not silently degrade to `TypeRef::Named` or equivalent fallbacks. The hierarchy's "real and deterministic" ownership contract means missing data is a bug in the input, not a forward-compatibility case.

**Why:** Silently falling back masks broken enrichment data and weakens the guarantee that all ownership/grouping/cross-link facts are real. User caught this twice: once here for impl self_type_path, and it matches the broader "no silent fallbacks in lowering" memory.

**How to apply:** When constructing the supported-slice item hierarchy, any unresolvable reference to a struct/type that should exist in the enrichment should be a hard error. Forward-compat `TypeRef::Named` is only for genuinely external types not expected in the enrichment.
