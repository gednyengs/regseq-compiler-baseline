# Zephyr Emulation Target Selection

## Purpose

This note records the current best candidate Zephyr/QEMU targets for validating
the two paper devices:

- PL011
- E1000

The goal is to avoid hand-wavy cross-OS claims by identifying concrete Zephyr
emulation targets early.

## Primary Finding

The strongest first Zephyr/QEMU target is:

- `qemu_cortex_a53`

Why:

- Zephyr documents this board as exposing:
  - `ARM PL011 UART` (`arm,pl011`)
  - `Intel E1000 Ethernet controller` (`intel,e1000`)
  - PCIe host support

That makes it the best first target because it can potentially exercise both
paper devices under one Zephyr/QEMU board.

## Current Recommended Zephyr Board

### `qemu_cortex_a53`

Recommended first target for Zephyr backend bring-up and smoke validation.

Reasons:

- PL011-capable
- E1000-capable
- QEMU-supported in Zephyr
- reduces target fragmentation during early Zephyr work

Relevant references:

- Zephyr board documentation:
  - <https://docs.zephyrproject.org/latest/boards/qemu/cortex_a53/doc/index.html>
- Zephyr networking/QEMU Ethernet setup:
  - <https://docs.zephyrproject.org/latest/connectivity/networking/qemu_eth_setup.html>
- Zephyr `arm,pl011` binding:
  - <https://docs.zephyrproject.org/latest/build/dts/api/bindings/serial/arm%2Cpl011.html>

## Secondary Board Note

### `qemu_x86`

This board is useful as a fallback for E1000-oriented Zephyr experiments, but
it is not the preferred cross-device target.

Reason:

- it supports `intel,e1000`
- but its serial path is `ns16550`, not PL011

So it may still be useful later for:

- E1000-focused troubleshooting
- isolating Zephyr network-driver issues

but it is not the clean first board for the combined paper-device story.

## What This Finding Does And Does Not Prove

This finding establishes:

- there is a concrete Zephyr/QEMU board to prioritize
- that board can potentially host both of our modeled devices

It does not yet establish:

- that both driver paths are equally mature in Zephyr
- that both are benchmark-friendly
- that the exact evaluation methodology for Zephyr should mirror Linux in full

Those are later validation questions.

## Immediate Recommendation

For Zephyr backend planning:

- prioritize `qemu_cortex_a53` first
- treat it as the default Zephyr bring-up and smoke-validation board
- only branch to other Zephyr QEMU boards if a concrete limitation appears
