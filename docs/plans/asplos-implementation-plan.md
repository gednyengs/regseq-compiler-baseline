# ASPlOS Implementation Plan

The implementation plan is now centered on RegseqModel as the single compiler
artifact shared by frontend construction, platform binding, analysis, and
backend work.

## Current Flow

```
rustc HIR/MIR
    |  construction (Phase 1: declarations populated, bindings empty)
    |  performed by regseq-driver's rustc integration
    v
Regseq Model
    |  platform file parsing + join (Phase 2: bindings populated)
    v
Regseq Model (fully populated)
    |
    v
Internal Regseq-specific analyses
    |
    v
Backend code generators (Linux, Zephyr, ...)
```

## Current Emit Contract

The supported emit specs are:

- `model`
- `analysis`
- `driver`

Eventually, `driver` will be the default emit kind.
However, while RegseqModel is still under construction, `model` should temporarily
be the default emit kind.
Once RegseqModel is completely specified and ready for consumption by Regseq analyses
and backend codegens, `driver` should be set as the default emit kind.

## Near-Term Work

- Verify the live source-to-RegseqModel path end to end.
- Keep validation fixtures focused on RegseqModel.
- Reconnect analyses against `&RegseqModel`.
- Rebuild backend planning and emission against `&RegseqModel` and CFG bodies.
