# Representative Function Trace Validation

This note records Step 1 of the semantic procedural IR validation checklist:
manual tracing of representative `pl011-full-spec` functions into the proposed
model from `~/.claude/plans/happy-crafting-muffin.md`.

The goal is to verify that the proposed:

- item hierarchy
- `FunctionKind`
- `FunctionRole`
- `SemanticBody`

can represent the supported driver slice without MIR/Rust residue.

## Inputs

- [pl011-full-spec-mir-study.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/pl011-full-spec-mir-study.md)
- [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- final research plan at `~/.claude/plans/happy-crafting-muffin.md`

## Validation Results

### 1. `uart_clock_hz`

Source:
- [driver.rs:175](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs#L175)

Expected model placement:
- `Item::Impl(ImplDef)` for `impl Pl011FullSpec`
- `FunctionDef`
- `FunctionKind::InherentMethod { owner: DriverStructId }`
- `FunctionRole::Helper`

Expected semantic lowering:

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

Verdict:
- the proposed IR represents this helper cleanly
- `ResourcePropertyRead + WithDefault + Return(Some(...))` is sufficient
- no MIR residue is required

### 2. `set_baud_rate`

Source:
- [driver.rs:105](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs#L105)

Expected model placement:
- `Item::Impl(ImplDef)` for `impl Pl011FullSpec`
- `FunctionDef`
- `FunctionKind::InherentMethod { owner: DriverStructId }`
- `FunctionRole::Helper`

Expected semantic lowering:

```text
Let(div64, arithmetic expression over uart_clk_hz and baud_rate)
Let(ibrd, ...)
Let(fbrd, ...)
RegWrite("ibrd", [FieldWrite("BAUD_DIVINT", Local(ibrd))])
RegWrite("fbrd", [FieldWrite("BAUD_DIVFRAC", Local(fbrd))])
Return(None)
```

Key validation points:
- derived arithmetic temporaries map naturally to `SemanticLocal { origin: Derived }`
- wrapper deref + `field_val(...)` + `ReadWrite::write(...)` can be absorbed into
  `RegWrite`
- no backend needs to see wrapper syntax

Verdict:
- the proposed IR represents this helper cleanly
- no immediate design gap surfaced here

### 3. `enable_clock_if_present`

Source:
- [driver.rs:183](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs#L183)

Expected model placement:
- `FunctionDef`
- `FunctionKind::InherentMethod { owner: DriverStructId }`
- `FunctionRole::Helper`

Expected semantic lowering:

```text
Branch {
    condition: ResourcePresent(ResourceRef { kind: Clock, field_name: "clock" }),
    then_body: [
        FallibleOp {
            body: [ResourceOp { resource: clock_ref, operation: Enable }],
            error_local: ret,
            cleanup_on_error: [Return(Some(LocalRef(ret)))]
        }
    ],
    else_body: []
}
Return(Some(Ok/unit-success value in carrier form))
```

Verdict:
- the proposed IR is sufficient for optional resource gating plus `?`
- no MIR/Rust residue is required

Note:
- exact success-value encoding for `Result<(), RegseqError>` still depends on the
  final `FallibleOp`/return-carrier conventions, but that is already recognized
  by the plan as an error-model question rather than a structural gap

### 4. `toggle_reset_if_present`

Source:
- [driver.rs:198](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs#L198)

Expected model placement:
- `FunctionDef`
- `FunctionKind::InherentMethod { owner: DriverStructId }`
- `FunctionRole::Helper`

Expected semantic lowering:

```text
Branch {
    condition: ResourcePresent(ResourceRef { kind: Reset, field_name: "reset" }),
    then_body: [
        FallibleOp {
            body: [ResourceOp {
                resource: reset_ref,
                operation: Toggle { duration_us: Some(10) }
            }],
            ...
        }
    ],
    else_body: []
}
Return(success)
```

Verdict:
- the proposed IR is sufficient
- `Duration::from_us(10)` is absorbed into `Toggle { duration_us: Some(10) }`

### 5. `apply_default_pins_if_present`

Source:
- [driver.rs:206](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs#L206)

Expected model placement:
- `FunctionDef`
- `FunctionKind::InherentMethod { owner: DriverStructId }`
- `FunctionRole::Helper`

Expected semantic lowering:

```text
Branch {
    condition: ResourcePresent(ResourceRef { kind: PinCtrl, field_name: "pinctrl" }),
    then_body: [
        FallibleOp {
            body: [ResourceOp {
                resource: pinctrl_ref,
                operation: ApplyState { state: "DefaultState" }
            }],
            ...
        }
    ],
    else_body: []
}
Return(success)
```

Verdict:
- the proposed IR is sufficient

Non-blocking note:
- the lowering step must canonicalize the state name deterministically
  (`DefaultState`, `SleepState`) rather than leaving generic Rust syntax behind

### 6. `service_rx_fifo`

Source:
- [driver.rs:231](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs#L231)

Expected model placement:
- `FunctionDef`
- `FunctionKind::InherentMethod { owner: DriverStructId }`
- `FunctionRole::Helper`

Authored source intent:

```text
let mut handled = false;
while self.rx_char().is_some() {
    handled = true;
}
return handled;
```

Planned semantic lowering in the research plan:

```text
Let(handled, BoolLit(false))
Loop {
    condition: HelperCall("rx_char", []),
    body: [Assign(Local(handled), BoolLit(true))]
}
Return(Some(LocalRef(handled)))
```

Validation result:
- this is **not type-clean as written**
- `rx_char()` returns `Option<RxChar>`, not `bool`
- the proposed condition uses `HelperCall("rx_char", [])` directly as a loop
  condition, which relies on implicit truthiness for an option-like carrier

Discovered design gap:
- the semantic IR needs an explicit way to test presence on optional values in
  general, not just on resources

Candidate repairs:
- add an expression like `IsSome(Box<Expr>)`
- or add a more general carrier test expression for `Option`/`Result`

Verdict:
- the design almost handles this case
- but it currently needs an explicit optional-value presence test in the
  expression language

### 7. `service_tx_fifo`

Source:
- [driver.rs:240](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs#L240)

Expected model placement:
- `FunctionDef`
- `FunctionKind::InherentMethod { owner: DriverStructId }`
- `FunctionRole::Helper`

Authored source intent:

```text
if let Some(byte) = self.pending_tx {
    if self.tx_ready() {
        write DR.DATA = byte as u32;
        self.pending_tx = None;
        self.disable_tx_interrupts();
        return true;
    }
    return false;
}
self.disable_tx_interrupts();
return true;
```

Planned semantic lowering in the research plan:

```text
Branch {
    condition: DriverFieldRef("pending_tx"),
    then_body: [
        Let(byte, DriverFieldRef("pending_tx")),
        Branch {
            condition: HelperCall("tx_ready", []),
            then_body: [
                Let(byte_u32, Cast(LocalRef(byte), U32)),
                RegWrite("dr", [FieldWrite("DATA", Local(byte_u32))]),
                FieldAssign("pending_tx", BoolLit(false)),
                ...
            ],
            else_body: [Return(Some(BoolLit(false)))]
        }
    ],
    else_body: [...]
}
```

Validation result:
- this surfaces two real design issues

Discovered design gap 1:
- `DriverFieldRef("pending_tx")` is being used as an option-presence condition
- that has the same issue as `service_rx_fifo`: it is relying on implicit
  truthiness of an `Option<u8>`-like carrier rather than an explicit semantic
  presence test

Discovered design gap 2:
- `FieldAssign("pending_tx", BoolLit(false))` is not type-correct
- the authored assignment is `pending_tx = None`, not `false`
- the semantic IR therefore needs explicit option-like value constructors or
  carrier literals

Candidate repairs:
- add `Expr::IsSome(Box<Expr>)`
- add explicit carrier constructors such as:
  - `Expr::OptionNone`
  - `Expr::OptionSome(Box<Expr>)`
- or a more general algebraic-value representation for carriers

Verdict:
- the current design does **not yet fully represent** this function cleanly
- it needs explicit option-presence and option-value constructs

### 8. `init`

Source:
- [driver.rs:269](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs#L269)

Expected model placement:
- `Item::Impl(ImplDef)` for `impl Lifecycle for Pl011FullSpec`
- `FunctionDef`
- `FunctionKind::TraitMethod { trait_ref: Lifecycle, owner: DriverStructId }`
- `FunctionRole::LifecycleInit`

Expected semantic lowering:

```text
FallibleOp(toggle_reset_if_present()?)
FallibleOp(apply_default_pins_if_present()?)
FallibleOp(enable_clock_if_present()?)
Let(clk, HelperCall("uart_clock_hz", []))
Eval(HelperCall("program_polling_port", [LocalRef(clk), DriverFieldRef("current_speed")]))
Return(success)
```

Verdict:
- the proposed IR represents this callback cleanly
- it validates:
  - trait-method ownership
  - lifecycle semantic role
  - fallible sequencing
  - helper calls with value arguments

### 9. `handle_irq`

Source:
- [driver.rs:375](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs#L375)

Expected model placement:
- `FunctionDef`
- `FunctionKind::InherentMethod { owner: DriverStructId }`
- `FunctionRole::Isr { binding: "irq" }`

Expected semantic lowering:

```text
Let(status, RegReadRaw("mis"))
Branch(status == 0) -> Return(NotHandled)
Let(handled, false)
Branch(InterruptClassTest(status, "rx")) -> handled |= HelperCall("service_rx_fifo", [])
Branch(InterruptClassTest(status, "tx")) -> handled |= HelperCall("service_tx_fifo", [])
Eval(HelperCall("clear_explicit_interrupts", [LocalRef(status)]))
Branch(LocalRef(handled)) -> Return(Handled) else Return(NotHandled)
```

Verdict:
- the proposed IR represents this ISR cleanly
- ISR structural ownership and ISR semantic role remain orthogonal and
  unambiguous
- no new design gap surfaced here

## Step 1 Verdict

Step 1 is **completed**, and it produced useful findings.

Representative coverage status:

- `uart_clock_hz`: clean
- `set_baud_rate`: clean
- `enable_clock_if_present`: clean
- `toggle_reset_if_present`: clean
- `apply_default_pins_if_present`: clean
- `service_rx_fifo`: gap discovered
- `service_tx_fifo`: gap discovered
- `init`: clean
- `handle_irq`: clean

## Discovered Design Gaps

### Gap 1: explicit optional-value presence tests are missing

The current expression language has:
- `ResourcePresent { resource }`

But it does not have a general optional-value presence test for:
- helper results like `rx_char().is_some()`
- driver fields like `pending_tx`
- local option-typed values in general

This caused both:
- `service_rx_fifo`
- `service_tx_fifo`

to become type-unclear in the proposed lowering.

Required direction:
- add a general expression such as `Expr::IsSome(Box<Expr>)`
- or another explicit carrier-test construct

### Gap 2: explicit option-like carrier values are missing

The current statement/expression design does not have a clean way to represent:
- `pending_tx = None`
- and, by extension, other option-like carrier assignments

This surfaced in the `service_tx_fifo` trace where the sketched lowering used:
- `FieldAssign("pending_tx", BoolLit(false))`

which is not type-correct.

Required direction:
- add explicit carrier-value constructs, likely:
  - `Expr::OptionNone`
  - `Expr::OptionSome(Box<Expr>)`
- or a more general tagged-carrier expression family

## Recommendation Before Step 2

Carry these two gaps forward into the next validation stages:

1. HIR/MIR Coverage Matrix
2. Acceptance Example Set

The current plan is still strong overall, but these option-carrier issues should
be treated as real semantic-IR design work, not emission details.
