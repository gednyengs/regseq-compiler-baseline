# ASPlOS Implementation Plan

The implementation plan is now centered on RegseqModel as the single compiler
artifact shared by frontend construction, platform binding, analysis, and
backend work.

## Current Flow

1. Rust source
2. rustc HIR/MIR
3. Phase-1 `RegseqModel`
4. platform-populated `RegseqModel`
5. RegseqModel JSON

## Current Emit Contract

The supported emit specs are:

- `model`
- `driver`
- `analysis`

`model` is the default emit kind. `driver` and `analysis` are reserved for the
backend/analysis reconnect work and are currently stubbed in `regseq-driver`.

## Near-Term Work

- Verify the live source-to-RegseqModel path end to end.
- Keep validation fixtures focused on RegseqModel JSON.
- Reconnect analyses against `&RegseqModel`.
- Rebuild backend planning and emission against `&RegseqModel` and CFG bodies.
