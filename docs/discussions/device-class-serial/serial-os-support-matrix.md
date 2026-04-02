# Serial OS Support Matrix

## Purpose

This matrix records how the stable v1 Serial device-class spec lowers across
the audited OSes.

Evidence levels:

- `framework` = supported by Phase A framework analysis
- `challenger` = supported by concrete device challengers
- `mixed` = framework + challenger support

## Matrix

| Surface / concern | Linux | Zephyr | Windows | FreeBSD | macOS | Notes |
|---|---|---|---|---|---|---|
| `Serial` core trait | yes (`mixed`) | yes (`mixed`) | yes (`framework`) | yes (`mixed`) | yes (`framework`) | All 5 OSes have a clear lowerable serial core. |
| `SerialConsole` split | yes (`mixed`) | yes (`framework`) | yes (`framework`) | yes (`mixed`) | yes (`framework`) | Zephyr delegates to polled path; others have distinct console paths. |
| `port_open` | clear | clear | clear | clear | clear | Zephyr may generate a trivial/static implementation. |
| `port_close` | clear | trivial/no-op | clear | clear | clear | Same semantic role even where folded into static device model. |
| `configure` | clear | clear | clear | clear | clear | All 5 OSes expose baud/framing/flow-control configuration. |
| `tx_byte` / `rx_char` | clear (`challenger`) | clear (`challenger`) | clear (`framework`) | clear (`challenger`) | clear (`framework`) | Byte primitives are validated across PL011, NS16550, STM32. |
| `tx_ready` | clear | clear | clear | clear | clear | Universal transmitter-space notion. |
| `tx_empty` | clear (`challenger`) | clear (`challenger`) | clear (`framework`) | clear (`challenger`) | partial (`framework`) | Strongly evidenced on Linux/Zephyr/FreeBSD; weakly documented on macOS. |
| `flush_rx_fifo` | clear (`challenger`) | clear (`challenger`) | clear (`framework`) | clear (`challenger`) | clear (`framework`) | May noop, combine, or be unused depending on device/backend. |
| `flush_tx_fifo` | clear (`challenger`) | clear (`challenger`) | clear (`framework`) | clear (`challenger`) | clear (`framework`) | Same as RX flush. |
| `send_break` | clear (`challenger`) | clear (`challenger`) | clear (`framework`) | clear (`challenger`) | clear (`framework`) | Exercised most strongly on Linux family challengers. |
| `RxChar.errors` reconstruction | clear | clear | clear | clear | clear with backend work | macOS is the weakest direct match, but still lowerable. |
| `SerialErrors` baseline (`overrun/parity/framing/break`) | clear | clear | clear | clear | clear | Stable v1 set. |
| `noise` handling | backend-private | surfaced in framework/device | framework-unclear | framework-unclear | framework-unclear | Audited as real hardware concern, not stable v1 public bit yet. |
| flow control as config/capability | clear | clear | clear | clear | clear | No separate v1 trait required. |
| modem/control as Layer 2 + lowering | clear | clear when present | clear | clear | clear | Some OSes make it more central, but not enough for a separate v1 trait. |
| `Clock` dependency through external construct | clear | clear | device-dependent | clear | device-dependent | Compiler must treat as operational dependency where hardware needs it. |
| DMA outside core `Serial` | clear | clear | clear | clear | clear | Linux/Zephyr use DMA; others omit it; no v1 trait needed. |
| RS-485 outside core `Serial` | clear | clear | no-op | no-op | no-op | Remains extension territory with backend-specific lowering. |

## Device-family evidence summary

| Device family | Linux | Zephyr | FreeBSD | Main contribution |
|---|---|---|---|---|
| PL011 | yes | yes | yes | confirmed byte core, `tx_empty`, console split, clock dependency |
| NS16550 / 8250 | yes | yes | yes | confirmed `tx_empty`, `send_break`, modem richness without new traits, FIFO hooks remain valid |
| STM32 USART | yes | yes | no local driver | confirmed byte core again, reinforced `tx_empty`, surfaced `noise` as a real hardware concern, strengthened hardware-managed RS-485 evidence |

## Main compiler takeaways

1. The stable v1 spec is lowerable on all five audited OSes.
2. Linux, Zephyr, and FreeBSD now have challenger-backed confirmation on the
   core data-plane hooks.
3. Windows and macOS remain framework-backed rather than challenger-backed, but
   the support picture is still strong enough for compiler implementation.
4. The two places where the compiler needs the most care are:
   - modem/error adaptation into OS-specific framework surfaces
   - console-safe lowering and context handling
