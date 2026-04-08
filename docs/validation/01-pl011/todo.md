# PL011 TODO

This file records implementation follow-up surfaced during the PL011 validation
pass.

Unlike [findings.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/validation/01-pl011/findings.md),
which records boundary-validation issues and evidence, this file tracks
compiler/spec work we expect to turn into harness tasks.

## Status Values

- `open`
- `in progress`
- `completed`
- `deferred`

## Urgent

### TODO-PL011-01

- Priority: `urgent`
- Area: authored spec correctness
- Summary:
  - add an explicit `#[rseq::reset(...)]` attribute to the reset resource field
    in the PL011 full-spec driver
- Why it matters:
  - reset is currently being treated as a resource field without explicit authored
    declaration, which weakens the intended field-classification rule
- Current workaround:
  - authored-source validation records it as a reset resource joined by field name
- Recommended task:
  - add the explicit reset attribute in the fixture/spec and update downstream
    expectations accordingly
- Status: `open`

### TODO-PL011-02

- Priority: `urgent`
- Area: driver-field classification
- Summary:
  - implement primary classification of driver struct fields by explicit
    `#[rseq::...(...)]` field attribute
- Why it matters:
  - Regseq fields and ordinary user fields need a clear source of truth
  - type-only classification is too heuristic
- Current workaround:
  - validation is treating explicit `rseq` field attributes as the intended
    classification rule
- Recommended task:
  - add extraction/model-side field classification logic driven by explicit
    field attributes
- Status: `open`

### TODO-PL011-03

- Priority: `urgent`
- Area: driver-field validation
- Summary:
  - implement secondary validation that the Rust type matches the declared
    `#[rseq::...(...)]` field kind
- Why it matters:
  - explicit classification alone is not enough; the compiler must reject
    mismatched declarations such as `#[rseq::clock]` on an incompatible type
- Current workaround:
  - authored specs must stay disciplined manually
- Recommended task:
  - add field-kind/type compatibility checks in the extraction or normalization
    path
- Status: `open`

## Deferred

### TODO-PL011-04

- Priority: `deferred`
- Area: user-field negative validation
- Summary:
  - implement a recursive negative check for unannotated user fields whose type
    tree embeds reserved Regseq resource-carrier types
- Why it matters:
  - a field like `Option<Clock>` or a deeper nested embedding should not be
    silently accepted as ordinary user state if it actually carries reserved
    Regseq resource identity
- Current workaround:
  - author specs correctly and avoid such patterns for now
- Recommended task:
  - walk reachable type projections from the root field type and reject
    unannotated embeddings of reserved Regseq resource-carrier types
- Status: `deferred`

### TODO-PL011-05

- Priority: `deferred`
- Area: pinctrl state typing
- Summary:
  - replace the current pinctrl state-name heuristic with a typed closed
    Layer-1 `PinStateMode` design
- Why it matters:
  - the current lowering path maps pinctrl state types by string heuristic
    (for example, names containing `"sleep"` map to sleep and everything else
    maps to default), which is brittle
  - Layer 1 should express a closed semantic state set, while Layer 2 should
    provide system-level realization of those states
- Current workaround:
  - authored specs use `DefaultState` and `SleepState`, which happen to match
    the current heuristic
- Recommended task:
  - introduce a compiler-known closed `PinStateMode` enum in the Regseq facade
  - require `PinState` marker types to provide an associated constant such as
    `const MODE: PinStateMode`
  - extract and lower pinctrl state usage through that typed semantic mode
    instead of string matching on state type names
- Status: `deferred`
