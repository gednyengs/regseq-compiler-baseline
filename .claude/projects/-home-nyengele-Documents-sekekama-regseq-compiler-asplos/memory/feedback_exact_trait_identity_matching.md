---
name: Exact canonical trait identity matching
description: Trait callback classification must use exact canonical paths, not suffix/prefix heuristics
type: feedback
---

When classifying function roles based on trait identity (e.g., Lifecycle vs Serial), use exact canonical trait path matching against registered known paths, not `ends_with("::Lifecycle")` or similar suffix patterns.

**Why:** Suffix matching on canonical paths is still a heuristic. If the enrichment provides canonical trait identity from the compiler, the classification should key off the exact resolved identity. Two different traits could share a suffix.

**How to apply:** Accept known canonical trait paths as explicit configuration input (e.g., a struct with fields for each supported trait family). Match `trait_path` exactly against these registered paths. Let the caller provide the correct paths from the regseq crate's known exports.
