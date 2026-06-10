# Surface note — `regseq::log::{debug, info, warn, error}!` (2026-06-10)

Status note for the authored logging spelling, landed as task T01 of the
`backend-linux-c` unit (GRILL D10 / PRD R05). This is a **facade-only**
promotion — no recognition or lowering change.

## The canonical authored spelling

```rust
regseq::log::debug!("template {}", arg);
regseq::log::info!("template {:#x}", arg);
regseq::log::warn!("template");
regseq::log::error!("template {} {}", a, b);
```

Each macro expands to the already-recognized canonical intrinsic form — a call
to the per-level logging anchor with the `template + LogArg[]` shape (the
`#[rseq::intrinsic("log-{level}")]` anchors). Recognition
(`regseq-driver`'s diag-log bridge) and every backend lowering see exactly the
same MIR as before; the macros are spelling, not semantics.

## What changed (and what did not)

- The driver-fixture author surface (`tests/fixtures/*/src/regseq/log.rs`)
  already re-exported the level macros under the namespaced path
  (`pub use log_info as info;` etc.). That namespaced form is now declared the
  canonical authored spelling; the crate-root `log_*!` names remain as the
  `#[macro_export]` mechanics behind it.
- `tests/fixtures/log-intrinsic-smoke` is rewritten through the namespaced
  spelling. The equivalence lock (R05's scope-creep guard) is structural:
  `regseq::log::{level}!` is a `pub use` alias of the *same* `macro_rules!`
  body (`src/regseq/log.rs`), so post-expansion tokens are identical by
  construction; the fixture crate compiles clean under the new spelling, with
  zero diffs under the recognition/backend crates and zero golden churn.
  (The fixture's historical consumer,
  `crates/regseq-driver-old/tests/construction_log_metadata.rs`, is archived
  outside the workspace and no longer builds — no live test compiles this
  fixture today.)
- The coverage-corpus fixtures (`crates/codegen-coverage-runner/fixtures/diag_*`)
  already author through inline `regseq::log::{level}!` macros of the same
  expansion shape — no change there.

## Relation to the open design discussion

`logging-and-printing-discussion.md` (status: open, leading candidate) already
named the destination: one canonical anchor per level, string-literal template,
typed Regseq-owned argument carriers. This note records that the *authored
spelling* over those anchors is now pinned to the namespaced macro form, as
required by the `pl011-composed` fixture and the Linux/Zephyr backend lowering
specs (which map level → `pr_*`/`dev_*` and `LOG_*` macro families
respectively). The anchor/carrier design itself is unchanged.
