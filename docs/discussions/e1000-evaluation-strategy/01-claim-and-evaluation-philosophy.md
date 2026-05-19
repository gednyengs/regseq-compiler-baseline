# 01 — Claim and Evaluation Philosophy

## 1. What contribution is under evaluation

The paper's contribution is a **device-driver specification DSL and its
compiler**. E1000 (Intel 82540EM) is a *case study* through which that
contribution is demonstrated; it is not the object of study. The evaluation
must therefore answer "is this DSL+compiler a sound way to produce real
drivers?" — not "is this NIC driver fast?"

A specification-DSL contribution is defended along four criteria, in priority
order:

1. **Expressiveness** — the DSL can express a real, non-trivial driver's
   behavior without escaping to hand-written code for the parts that matter.
2. **Correctness** — the generated driver actually works the way a real driver
   works in the use cases that matter in practice.
3. **Portability** — *one* specification yields working drivers on *every*
   backend the paper claims (Linux **and** Zephyr). This is the headline
   contribution of a driver DSL: write once, lower everywhere.
4. **Acceptable cost** — the generated driver does not pay a pathological
   price relative to the hand-written driver.

## 2. Performance is defensive, not offensive

Criterion 4 is a **threshold, not a curve**. The paper does not claim Regseq
produces a faster NIC driver, and it does not need to. It needs to show the
generated driver does not regress catastrophically against the native driver
on a coarse, honest measure. Consequently:

- The evaluation reports **one** sustained-throughput and **one**
  CPU-cost-per-byte comparison versus native, framed explicitly as a
  "no-pathological-regression" guardrail.
- It does **not** pursue microarchitectural performance discrimination
  (interrupt-moderation cadence, descriptor-ring fill/drain efficiency,
  small-packet latency curves). Those characterize the device and the emulator,
  not the DSL, and — as shown in §03 — cannot even be measured honestly on the
  evaluation substrate.

Stating this boundary explicitly is itself part of the methodology: the most
common way a systems-artifact evaluation overclaims is by promoting a
defensive guardrail into an offensive result.

## 3. The single central claim the evaluation must defend

> One semantic Intel 82540EM specification, authored once in Regseq, compiles
> to Linux and Zephyr drivers that (a) build and integrate with each OS's
> networking subsystem and (b) preserve native-driver behavior on realistic
> NIC use-case workloads, at no pathological cost.

Offensive sub-claims: **expressiveness** (the spec is a compact, declarative
artifact that captures a real NIC driver) and **portability** (one spec → two
working OS drivers with divergence isolated to backend lowering). Defensive
sub-claim: **cost** is bounded.

The cross-OS **portability** claim rests only on the workloads that are
*fully comparable on both Linux and Zephyr* — the headline rows W1–W3 (§02 §3).
The Linux-primary correctness workloads W4–W5 corroborate correctness but are
**not** presented as cross-OS portability evidence, and the paper's table
structure (§02 §5) keeps that separation explicit so the headline claim is not
read off Linux-only rows.

## 4. QEMU-relative scope and its honest limits

All measurement is QEMU-relative: native and Regseq drivers run against the
*same* emulated device model, and only the driver under test differs. This
supports relative correctness and relative cost statements. It does **not**
support absolute-hardware performance, codegen-quality, or device-fidelity
claims, and it cannot support any claim about a device feature the emulator
does not faithfully model. The strategy therefore restricts the offensive
claims to properties the substrate can honestly exhibit (functional behavior,
cross-OS parity) and confines performance to a guardrail (§02, §03).

## 5. Relationship to the taxonomy / representability thesis

This evaluation is deliberately complementary to the taxonomy thesis, which is
*representability-scoped*: the thesis establishes that the locked taxonomy/DSL
can **express** PL011 and E1000. This compiler-side evaluation establishes that
the expressed E1000 spec, once compiled, produces **drivers that actually work
cross-OS in practical use**. Together they form one coherent two-repository
story — *representable* (thesis) and *realizable cross-OS* (compiler) — with no
claim that exceeds what the QEMU-relative substrate can support. Nothing here
reintroduces an implemented-compiler, codegen, or empirical-saturation claim
that the thesis explicitly disclaims.

## 6. Disposition of the prior locked decisions

| Locked decision | Disposition under this strategy |
| --- | --- |
| D1 (Linux topology) | **Revised**: x86_64 guest + KVM (was aarch64 `virt`/TCG). KVM gives one believable low-variance cost figure and fast functional iteration; substrate parity with PL011 is no longer load-bearing once the P-matrix is cut. |
| D2 (Zephyr topology) | Retained: `qemu_cortex_a53` + `QEMU_CORTEX_A53_ETH_E1000`, PL011 control channel. |
| D3 (four-dimension per-OS analysis) | Retained as evidence-gathering for the spec; no longer feeds a full axis matrix. |
| D4 (use cases U1–U7) | Retained. The minimal workload set in §02 maps onto U1–U7; no use case is dropped, only the over-fine axis decomposition is. |
| D5 (anti-gaming: axes derive from use cases) | Retained and strengthened: the workload set derives from U1–U7, not from what Regseq or QEMU can do. |
| D6 (RX/TX checksum claim-path) | **Reframed**: TX checksum survives as a functional-correctness observation folded into bulk transfer; RX-checksum offload is cut as a runtime row (QEMU does not model it — §03). The DSL still *expresses* checksum offload — that is expressiveness evidence, not a benchmarked row. |
| D7 (interrupt moderation claim-path) | **Cut as a runtime row** (QEMU does not faithfully model it; 82540EM exposes no native ethtool coalesce interface — §03). Moderation registers remain *expressible* in the spec (expressiveness), with no runtime-benchmark obligation. |
| D8 / D9 / D10 (VLAN / flow-control / jumbo deferred) | Retained unchanged. |

The reframing of D6/D7 removes a feature from the *measurement matrix*, not
from the *spec or the expressiveness claim*. A reviewer must be able to see
this distinction plainly; it is made explicit in §02 §1 and §03 §4.
