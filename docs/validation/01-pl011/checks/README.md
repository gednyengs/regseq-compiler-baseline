# PL011 RegseqModel Checks

The previous extraction-boundary check contracts were retired. New PL011
validation work should target the live compiler boundary:

1. authored Rust source
2. rustc HIR/MIR as consumed by `regseq-driver`
3. Phase-1 `RegseqModel`
4. platform-populated `RegseqModel`
5. future analysis/backend outputs once those emitters are connected

Use PL011 as a proving fixture only. Compiler changes must generalize to
structurally similar drivers and must not introduce PL011-specific or
OS-specific shortcuts.
