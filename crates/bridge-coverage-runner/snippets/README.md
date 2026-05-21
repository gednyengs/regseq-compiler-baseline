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
