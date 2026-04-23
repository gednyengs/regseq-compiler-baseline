# PL011 Cross-OS Feature Matrix

This note summarizes the high-level PL011 hardware functionalities
captured by the four PL011 drivers in the current corpus:

- Linux
- FreeBSD
- Zephyr
- Windows 10 IoT Core (SerCx2 WDF driver; added after this matrix's
  original three-OS version was written)

The matrix tables below still reflect the original three-OS
comparison. Windows features are captured in
`pl011-comparison-axes.md` and `pl011-spec-requirements.md` via
the outcome-oriented axes derivation; updating this matrix to a
four-column form is housekeeping only and does not change the
paper claim.

The goal is to identify:

- the common PL011 hardware baseline
- the maximal feature set evidenced anywhere in the corpus
- which OS currently contributes each feature

## Feature matrix

Legend:

- `Y` = clearly implemented
- `P` = partial / conditional / limited
- `N` = not implemented in the corpus driver

| Hardware functionality                           | Linux | FreeBSD | Zephyr | Notes                                                                                               |
| ------------------------------------------------ | ----- | ------- | ------ | --------------------------------------------------------------------------------------------------- |
| Polled TX/RX data path                           | Y     | Y       | Y      | All three support low-level character I/O for boot/console/debug style use.                         |
| Interrupt-driven TX/RX data path                 | Y     | Y       | Y      | All three support IRQ-driven FIFO service.                                                          |
| Baud-rate programming                            | Y     | Y       | Y      | All three program integer/fractional divisors.                                                      |
| Line-format configuration                        | Y     | Y       | Y      | All three cover word length / parity / stop bits.                                                   |
| FIFO enable/disable and threshold control        | Y     | Y       | Y      | All three manage FIFO mode and interrupt thresholds.                                                |
| UART enable/disable sequencing                   | Y     | Y       | Y      | All three implement explicit enable/disable ordering.                                               |
| Error detection on RX path                       | Y     | Y       | Y      | All three inspect error conditions during receive handling.                                         |
| Hardware revision / variant handling             | Y     | Y       | Y      | Linux is richest; FreeBSD and Zephyr also include variant or revision logic.                        |
| Clock integration / clock-derived baud logic     | Y     | P       | Y      | Linux and Zephyr actively integrate clocks; FreeBSD mainly infers clock or consumes firmware state. |
| Interrupt mask lifecycle / selective IRQ control | Y     | Y       | Y      | All three manage interrupt mask state explicitly.                                                   |
| DMA TX                                           | Y     | N       | N      | Linux only in current corpus.                                                                       |
| DMA RX                                           | Y     | N       | N      | Linux only in current corpus.                                                                       |
| Hardware flow control (RTS/CTS)                  | Y     | N       | Y      | FreeBSD does not implement it.                                                                      |
| Modem status signals / modem control             | Y     | N       | N      | Linux only in current corpus.                                                                       |
| RS-485 half-duplex support                       | Y     | N       | N      | Linux only in current corpus.                                                                       |
| Break generation / break control                 | Y     | N       | N      | Linux only in current corpus.                                                                       |
| Loopback exposure                                | P     | N       | N      | Linux exposes it through generic serial control paths; others do not.                               |
| Pinctrl integration                              | Y     | N       | Y      | Linux and Zephyr integrate pin configuration; FreeBSD corpus driver does not.                       |
| Reset-controller integration                     | N     | N       | Y      | Zephyr only in current corpus.                                                                      |
| Suspend/resume or PM-aware register preservation | Y     | N       | Y      | Linux and Zephyr have real PM-aware handling.                                                       |
| Console / early-console special paths            | Y     | Y       | P      | Strongest in Linux, present in FreeBSD, less central in Zephyr corpus driver.                       |
| Board / SoC-specific hooks or quirks             | Y     | Y       | Y      | All three have some platform-specific adaptation logic.                                             |

## Common baseline across all three OSes

The common PL011 baseline evidenced in Linux, FreeBSD, and Zephyr is:

1. polled TX/RX
2. interrupt-driven TX/RX
3. baud-rate programming
4. line-format configuration
5. FIFO control and trigger programming
6. UART enable/disable sequencing
7. RX-path error handling
8. interrupt mask and status lifecycle
9. hardware revision / variant handling

This is the strongest candidate for the **mandatory core** of a unified
PL011 Regseq spec.

## Maximal feature set evidenced in the corpus

The maximal PL011 hardware/integration feature set evidenced anywhere in
the current corpus is:

1. polled TX/RX
2. interrupt-driven TX/RX
3. baud-rate programming
4. line-format configuration
5. FIFO control and trigger programming
6. UART enable/disable sequencing
7. RX-path error handling
8. interrupt mask and status lifecycle
9. hardware revision / variant handling
10. clock integration
11. DMA TX
12. DMA RX
13. hardware RTS/CTS flow control
14. modem status and modem control lines
15. RS-485 half-duplex support
16. break generation
17. pinctrl integration
18. reset-controller integration
19. suspend/resume or PM-aware state preservation
20. console / early-console pathways
21. board / SoC-specific quirk and hook support

## Immediate synthesis takeaways

- Linux contributes most of the maximal feature set.
- Zephyr contributes a strong embedded-oriented feature profile:
  clocking, reset, pinctrl, flow control, PM-aware save/restore, and
  variant hooks.
- FreeBSD contributes a lean but credible baseline and useful platform
  quirk evidence, even where it does not implement advanced features.

## Why this matters for a unified Regseq PL011 spec

The matrix supports a strong reuse story:

- the unified spec does not need to be limited to the weakest current
  driver
- instead, it can encode the maximal feature set evidenced anywhere in
  the corpus
- once encoded, thinner OS implementations can potentially inherit
  those features through backend lowering rather than through fresh
  manual rediscovery of PL011 hardware behavior
