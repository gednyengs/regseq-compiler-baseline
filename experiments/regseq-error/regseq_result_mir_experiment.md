# RegseqResult MIR Experiment

## Purpose

This experiment isolates the three error-shape cases we are debating under a
custom `RegseqResult<T>` carrier:

1. explicit authored semantic error
2. wrapper-mediated `?` propagation
3. wrapper-mediated mapping from native status into `RegseqResult`
4. native-status passthrough analogue

The goal is to see whether a custom Regseq result carrier gives the compiler a
cleaner MIR signal than ordinary `Result<T, RegseqError>`.

## Files

- [`regseq_result_mir_experiment.rs`](./regseq_result_mir_experiment.rs)

## How To Run

From the repo root:

```sh
rustc experiments/regseq-error/regseq_result_mir_experiment.rs \
  --crate-type lib \
  -Zunpretty=mir
```

To narrow the output:

```sh
rustc experiments/regseq-error/regseq_result_mir_experiment.rs \
  --crate-type lib \
  -Zunpretty=mir | rg -n "explicit_authored_error|wrapper_mediated_error|native_status_passthrough|RegseqResult|branch|from_residual|enable_native"
```

## What To Look For

### 1. Explicit authored error

Expected signal:

- direct construction of `RegseqError::Timeout`
- direct construction of `RegseqResult::Err(...)`

This is the authored-semantic case.

### 2. Wrapper-mediated `?`

Expected signal:

- call to `Clock::enable`
- `Try::branch` on `RegseqResult`
- `FromResidual` path rebuilding `RegseqResult`

This is the key case for deciding whether `RegseqResult` should be the sole DSL
fallible carrier.

### 3. Native-status passthrough analogue

### 3. Wrapper-mediated mapping from native status

Expected signal:

- call to `Clock::enable_mapped`
- wrapper body calls `enable_native`
- native status branch occurs inside the wrapper
- wrapper constructs `RegseqResult::Err(RegseqError::Busy)` on failure
- caller still sees `Try::branch` over `RegseqResult`

This is the key experiment for the wrapper-boundary decision. It shows whether
wrapper-mediated native failures become indistinguishable from explicitly
semantic authored failures once they are mapped into the DSL carrier.

### 4. Native-status passthrough analogue

Expected signal:

- call to `Clock::enable_native`
- branch on a native status value
- direct propagation of that status

This shows the shape of a native failure path when it is not modeled as a DSL
semantic result.

## Working Hypothesis

If this experiment shows clearly distinguishable MIR shapes for the three
cases, then a custom `RegseqResult<T>` carrier is a plausible strategy for:

- making authored semantic propagation explicit
- distinguishing authored Regseq failure flow from ordinary Rust `Result`
- keeping native-status passthrough outside the authored DSL carrier

If the MIR signal is not materially better than ordinary `Result`, then the
complexity of a custom `Try` carrier is harder to justify.
