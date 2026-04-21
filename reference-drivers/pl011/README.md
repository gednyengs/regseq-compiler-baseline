# ARM PrimeCell PL011 UART — Driver Sources

This folder collects publicly available driver source code for the ARM PrimeCell
PL011 UART across four operating systems. Each subdirectory contains files
fetched verbatim from the upstream repository on 2026-04-20.

Licenses differ per project — see each subdirectory's headers / LICENSE files.

## Contents

### `linux/` — Linux kernel driver (GPL-2.0+)
- `amba-pl011.c` — main driver (3,223 lines)
  Source: https://github.com/torvalds/linux/blob/master/drivers/tty/serial/amba-pl011.c
- `serial_pl011.h` — PL011 register definitions (from `include/linux/amba/serial.h`)
  Source: https://github.com/torvalds/linux/blob/master/include/linux/amba/serial.h

Device tree binding lives in `Documentation/devicetree/bindings/serial/` in the
kernel tree; the exact file has moved between YAML refactors, so grab it fresh
from the kernel source when needed.

### `zephyr/` — Zephyr RTOS driver (Apache-2.0)
- `uart_pl011.c` — main driver (831 lines)
  Source: https://github.com/zephyrproject-rtos/zephyr/blob/main/drivers/serial/uart_pl011.c
- `uart_pl011_registers.h` — register map
  Source: https://github.com/zephyrproject-rtos/zephyr/blob/main/drivers/serial/uart_pl011_registers.h
- `arm_pl011.yaml` — devicetree binding
  Source: https://github.com/zephyrproject-rtos/zephyr/blob/main/dts/bindings/serial/arm%2Cpl011.yaml

### `freebsd/` — FreeBSD driver (BSD-2-Clause)
- `uart_dev_pl011.c` — main driver (612 lines)
  Source: https://github.com/freebsd/freebsd-src/blob/master/sys/dev/uart/uart_dev_pl011.c

### `windows/` — Windows 10 IoT Core driver for Raspberry Pi 2/3 (MIT)
Implemented as a SerCx2 (Serial WDF class extension) kernel-mode driver.
Source directory: https://github.com/ms-iot/rpi-iotcore/tree/master/drivers/uart/bcm2836/serPL011

27 files, including:
- `PL011driver.cpp` / `.h` — WDF driver entry points
- `PL011device.cpp` / `.h` — device object lifecycle
- `PL011hw.cpp` / `.h` — register-level hardware access
- `PL011uart.cpp` / `.h` — PL011-specific logic
- `PL011interrupt.cpp` / `.h` — ISR / DPC
- `PL011tx.cpp` / `.h`, `pl011rx.cpp` / `PL011rx.h` — TX/RX paths
- `PL011ioctl.cpp` / `.h` — ioctl dispatch
- `PL011common.cpp` / `.h`, `PL011logging.h`, `precomp.h` — shared helpers
- `SerPL011.inf`, `SerPL011.vcxproj`, `SerPL011.pkg.xml`, `sources.props`, `PL011.rc` — build / packaging
- `License.txt`, `README.md` — upstream project docs

## Notes on macOS

No open-source PL011 driver was found for macOS / Darwin. Apple Silicon Macs
use Apple's own UART (the s5l-style block), not PL011, so there is no in-tree
driver to collect. Asahi Linux has a separate driver (`serial-apple.c`) for that
Apple UART, unrelated to PL011.

## Licenses at a glance

| Project | License |
|---|---|
| Linux | GPL-2.0+ |
| Zephyr | Apache-2.0 |
| FreeBSD | BSD-2-Clause |
| Windows (ms-iot) | MIT |

Each file retains its original copyright header — check before redistributing.
