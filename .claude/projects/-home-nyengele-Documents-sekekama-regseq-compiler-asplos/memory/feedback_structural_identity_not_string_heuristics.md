---
name: Structural identity over string heuristics in extraction
description: Extraction must classify items and fields using HIR attributes and type resolution, never by pattern-matching type name strings
type: feedback
---

Extraction-time classification (e.g., resource_kind for struct fields) must use
structured HIR facts: `#[rseq::*]` attributes and rustc type resolution, not
display-string pattern matching on type names.

**Why:** The task architecture explicitly requires canonical identity derived
structurally from HIR/DefId facts. String heuristics violate the "no short-name
heuristics" constraint and create incidental coupling.

**How to apply:** When classifying fields, resolve via attributes first
(`#[rseq::mmio]`, `#[rseq::clock]`, etc.), then via rustc's type resolution
APIs. Never inspect the rendered type string to decide semantic role.
