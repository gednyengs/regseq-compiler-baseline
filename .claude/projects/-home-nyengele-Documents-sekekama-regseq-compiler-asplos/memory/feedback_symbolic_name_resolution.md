---
name: Symbolic name resolution over positional matching
description: Interrupt mask resolution must use symbolic name matching, not positional ordering
type: feedback
---

When resolving interrupt class mask values from MIR constants, match by symbolic name (e.g., constant path "RX_INTERRUPT_MASK" → class name "rx"), not by positional zip of collected masks against unresolved classes.

**Why:** Positional matching is brittle — if MIR order shifts, masks get silently misassigned. The MIR path already preserves symbolic constant paths via `unevaluated:...` and `derive_class_name_from_constant_path()`. Resolution should leverage this existing infrastructure.

**How to apply:** Extract constant paths from MIR operands, derive class names from paths, map class_name → mask_value directly. If unresolved classes remain after symbolic resolution, error explicitly rather than leaving mask_value = 0.
