# Acceptance Example Set

This note records Step 10 of the semantic procedural IR validation checklist.

The goal is to define the concrete “must work on paper” semantic examples that
the design must support cleanly before implementation begins.

These examples are not full test fixtures. They are the minimum semantic
contracts the design must satisfy.

## Inputs

- final research plan at `~/.claude/plans/happy-crafting-muffin.md`
- [representative-function-trace-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/representative-function-trace-validation.md)
- [hir-mir-coverage-matrix.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/hir-mir-coverage-matrix.md)

## Acceptance Examples

### 1. `uart_clock_hz`

Source intent:
- optional resource read with fallback constant

Required semantic lowering:

```text
Let(rate,
    WithDefault {
        value: ResourcePropertyRead {
            resource: ResourceRef { kind: Clock, field_name: "clock" },
            property: ClockRate,
        },
        default: ConstantRef(Pl011FullSpec::DEFAULT_UART_CLOCK_HZ),
    })
Return(Some(LocalRef(rate)))
```

Why this example matters:
- proves value-producing helper bodies work
- proves resource reads are semantic, not wrapper-shaped
- proves constant identity/defaulting are explicit

Acceptance condition:
- no backend must reconstruct `Option::as_ref/map/unwrap_or`

### 2. `set_baud_rate`

Source intent:
- arithmetic on inputs
- derived temporaries
- register writes via wrappers

Required semantic lowering:

```text
Let(div64, ...)
Let(ibrd, ...)
Let(fbrd, ...)
RegWrite("ibrd", [FieldWrite("BAUD_DIVINT", Local(ibrd))])
RegWrite("fbrd", [FieldWrite("BAUD_DIVFRAC", Local(fbrd))])
Return(None)
```

Why this example matters:
- proves derived MIR temps become semantic locals
- proves wrapper register accesses collapse into semantic register ops
- proves arithmetic and casts are preserved semantically

Acceptance condition:
- no raw MIR local or wrapper call survives into backend input

### 3. `handle_irq`

Source intent:
- raw status read
- interrupt-class tests
- helper dispatch
- accumulated handled result
- explicit clear behavior

Required semantic lowering:

```text
Let(status, RegReadRaw("mis"))
Branch(status == 0) -> Return(IRQ_NONE)
Let(handled, false)
Branch(InterruptClassTest(status, "rx")) -> handled |= HelperCall("service_rx_fifo", [])
Branch(InterruptClassTest(status, "tx")) -> handled |= HelperCall("service_tx_fifo", [])
Eval(HelperCall("clear_explicit_interrupts", [LocalRef(status)]))
Branch(LocalRef(handled)) -> Return(IRQ_HANDLED) else Return(IRQ_NONE)
```

Why this example matters:
- proves ISR semantics are first-class
- proves interrupt dispatch is semantic, not backend heuristic
- proves return-value selection can be modeled cleanly

Acceptance condition:
- backend does not infer ISR semantics from handwritten Linux templates

### 4. `init`

Source intent:
- fallible sequencing of helper calls
- one value-producing helper result fed into another helper call

Required semantic lowering:

```text
FallibleOp(toggle_reset_if_present()?)
FallibleOp(apply_default_pins_if_present()?)
FallibleOp(enable_clock_if_present()?)
Let(clk, HelperCall("uart_clock_hz", []))
Eval(HelperCall("program_polling_port", [LocalRef(clk), DriverFieldRef("current_speed")]))
Return(success)
```

Why this example matters:
- proves trait-method callback lowering
- proves fallible sequencing and helper composition
- proves value-producing helpers can feed later statements

Acceptance condition:
- backend does not see `Try::branch` / residual machinery

### 5. `service_tx_fifo`

Source intent:
- option-typed driver state
- option presence test
- option payload extraction
- register write
- state reset to `None`

Required semantic lowering:

```text
Branch {
    condition: IsSome(DriverFieldRef("pending_tx")),
    then_body: [
        Let(byte, OptionUnwrap(DriverFieldRef("pending_tx"))),
        Branch {
            condition: HelperCall("tx_ready", []),
            then_body: [
                Let(byte_u32, Cast(LocalRef(byte), U32)),
                RegWrite("dr", [FieldWrite("DATA", Local(byte_u32))]),
                FieldAssign("pending_tx", OptionNone),
                Eval(HelperCall("disable_tx_interrupts", [])),
                Return(Some(BoolLit(true)))
            ],
            else_body: [Return(Some(BoolLit(false)))]
        }
    ],
    else_body: [
        Eval(HelperCall("disable_tx_interrupts", [])),
        Return(Some(BoolLit(true)))
    ]
}
```

Why this example matters:
- proves option-carrier semantics are fully represented
- proves stateful driver fields are handled semantically
- proves presence test, unwrap, and `None` assignment all work together

Acceptance condition:
- no implicit truthiness
- no treating `None` as `false`
- no backend-side reconstruction of option semantics

## Overall Acceptance Contract

The semantic procedural IR design is acceptable for implementation only if all
five examples above can be lowered cleanly using:

- the proposed item hierarchy
- the proposed `FunctionKind` / `FunctionRole`
- the proposed `SemanticBody`, `Stmt`, and `Expr`
- the stated hard-error rules

without:

- raw MIR residue
- raw Rust path residue
- backend reconstruction of semantics
- short-name heuristics
- TODO/comment placeholders

## Step 10 Verdict

Step 10 is **completed**.

Outcome:

- the design now has an explicit acceptance example set
- these examples provide a concrete semantic “must work” contract for
  implementation planning
