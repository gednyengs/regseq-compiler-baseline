# Bridge Coverage Runner -- Snippets

Each `.rs` file under this directory is a one-shot bridge experiment
for a matrix row in
`docs/discussions/regseq-model/bridge-coverage-matrix.md`. File names
follow the matrix's row-id format -- `TY-018.rs` for the top-level
row `TY-018`, `RV-006-A.rs` for the sub-axis row `RV-006-A`,
`OP-003-G-1.rs` for the nested sub-row `OP-003-G-1`. Every snippet is
`#![no_std]` and authored to exercise the row's claimed construct in
a minimal shape. See
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
section 3 for the runner's full layout.

## Inspection-predicate `arg` string conventions

Most inspection predicates take a flat variant tag as their `arg`
string (e.g., `{ kind = "BinOp", arg = "Add" }`, `{ kind =
"PlaceElem", arg = "Deref" }`). One exception: the `CastKind`
predicate flattens the nested `CastKind::PointerCoercion(sub)`
shape into a dotted-compound argument
`"PointerCoercion::<sub>"`. A flat `"PointerCoercion"` arg would
collapse all four nested sub-variants ambiguously; the dotted
form preserves precision. Concretely:

- `{ kind = "CastKind", arg = "IntToInt" }` -- flat top-level
  variant.
- `{ kind = "CastKind", arg = "PointerCoercion::Unsize" }` --
  nested sub-variant of `PointerCoercion`.
- `{ kind = "CastKind", arg = "PointerCoercion::ReifyFnPointer" }`
  -- same dotted-compound shape for the other three
  sub-variants (`UnsafeFnPointer`, `ClosureFnPointer`,
  `Unsize`).
