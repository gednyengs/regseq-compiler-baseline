The Stage-1 list contains 40 REQ IDs, not 39. I audited all 40. Paths below are abbreviated to the requested basenames.

**Q1 Requirement Audit**

| REQ-ID | verdict | fixture-cite | datasheet-cite-if-relevant | notes |
|---|---|---|---|---|
| REQ-TX-01 | captured-correctly | driver.rs:303, driver.rs:339 | pl011-minimized-datasheet.md:837,876 | DR write guarded by TXFF. Req: pl011-spec-requirements.md:92 |
| REQ-TX-02 | captured-correctly | driver.rs:237,303,367 | pl011-minimized-datasheet.md:708 | Polled plus ISR TX path. Req: pl011-spec-requirements.md:95 |
| REQ-RX-01 | captured-correctly | driver.rs:314,319 | pl011-minimized-datasheet.md:837,877 | DR read guarded by RXFE. Req: pl011-spec-requirements.md:98 |
| REQ-RX-02 | captured-correctly | driver.rs:228,314,378 | pl011-minimized-datasheet.md:701 | Polled plus ISR RX path. Req: pl011-spec-requirements.md:101 |
| REQ-RX-03 | captured-correctly | driver.rs:322,334; registers.rs:8 | pl011-minimized-datasheet.md:832-837 | DR error bits propagated. Req: pl011-spec-requirements.md:104 |
| REQ-RX-04 | captured-correctly | driver.rs:319,336 | pl011-minimized-datasheet.md:826,861 | One DR read, then local decode. Req: pl011-spec-requirements.md:108 |
| REQ-CFG-01 | captured-with-issue | driver.rs:39,48,105 | pl011-minimized-datasheet.md:404-407,929-940 | Programs IBRD/FBRD, but divisor truncates instead of rounding. Req: pl011-spec-requirements.md:115 |
| REQ-CFG-02 | captured-correctly | driver.rs:160,162 | pl011-minimized-datasheet.md:1018-1025 | IBRD/FBRD before LCR_H. Req: pl011-spec-requirements.md:119 |
| REQ-CFG-03 | not-captured | driver.rs:120,295 | pl011-minimized-datasheet.md:1009-1016 | Hard-coded 8N1; SerialConfig ignored. Req: pl011-spec-requirements.md:124 |
| REQ-CFG-04 | captured-with-issue | driver.rs:160,166 | pl011-minimized-datasheet.md:1018-1025,1040-1042 | Ordering exists only for fixed 8N1 path. Req: pl011-spec-requirements.md:127 |
| REQ-FIFO-01 | captured-correctly | driver.rs:75,120,353 | pl011-minimized-datasheet.md:1012 | Uses LCR_H.FEN. Req: pl011-spec-requirements.md:133 |
| REQ-FIFO-02 | captured-correctly | driver.rs:95,98; registers.rs:144 | pl011-minimized-datasheet.md:1082-1094 | Static IFLS selection present. Req: pl011-spec-requirements.md:135 |
| REQ-FIFO-03 | captured-with-issue | driver.rs:347,355 | pl011-minimized-datasheet.md:1072-1078 | Hooks exist; TX flush lacks disable/wait sequencing. Req: pl011-spec-requirements.md:140 |
| REQ-LIFE-01 | captured-with-issue | driver.rs:68,128,156 | pl011-minimized-datasheet.md:1072-1078 | Enable/disable exists; no wait for current transfer before reprogram. Req: pl011-spec-requirements.md:147 |
| REQ-LIFE-02 | captured-with-issue | driver.rs:156,166 | pl011-minimized-datasheet.md:839-840,1040-1045 | Disable-before-reconfigure exists; safe wait/flush is missing. Req: pl011-spec-requirements.md:150 |
| REQ-IRQ-01 | captured-correctly | registers.rs:161,219; driver.rs:87,92 | pl011-minimized-datasheet.md:1096-1174 | IMSC/RIS/MIS/ICR modeled; ICR writes use ones. Req: pl011-spec-requirements.md:167 |
| REQ-IRQ-02 | captured-correctly | driver.rs:367,386 | pl011-minimized-datasheet.md:691,731 | ISR reads MIS, services RX/TX, clears errors. Req: pl011-spec-requirements.md:171 |
| REQ-IRQ-03 | not-captured | driver.rs:57,131,153 | n/a | No IMSC shadow; live IMSC RMW. Req: pl011-spec-requirements.md:174 |
| REQ-IRQ-04 | captured-with-issue | driver.rs:61,64,131,153; registers.rs:167,173 | pl011-minimized-datasheet.md:1109-1115 | RX/TX/RT/error bits named, but FE/PE/BE/OE masks are never enabled/disabled. Req: pl011-spec-requirements.md:178 |
| REQ-BRK-01 | captured-with-issue | driver.rs:358,363 | pl011-minimized-datasheet.md:1016,1043-1045 | BRK bit used; no two-frame hold or TX/FIFO ordering. Req: pl011-spec-requirements.md:188 |
| REQ-PERF-BATCH-01 | captured-with-issue | driver.rs:57,228,237 | n/a | RX drains loop; TX ISR is single-byte `pending_tx`. Req: pl011-spec-requirements.md:213 |
| REQ-PERF-LATENCY-01 | captured-with-issue | driver.rs:131,153 | n/a | Hot-path IMSC RMW remains. Req: pl011-spec-requirements.md:234 |
| REQ-ID-01 | captured-correctly | driver.rs:33 | pl011-minimized-datasheet.md:1208-1213 | `arm,pl011` compatible. Req: pl011-spec-requirements.md:243 |
| REQ-ID-02 | captured-correctly | driver.rs:33 | pl011-minimized-datasheet.md:94-96 | `bus = "amba"`. Req: pl011-spec-requirements.md:246 |
| REQ-RES-CLK-01 | captured-with-issue | driver.rs:48,66,172 | n/a | Optional clock exists; fallback clock is const, not property. Req: pl011-spec-requirements.md:254 |
| REQ-RES-CLK-02 | captured-correctly | driver.rs:179,191,264,278 | n/a | Clock enable/disable lifecycle present. Req: pl011-spec-requirements.md:257 |
| REQ-RES-RST-01 | captured-correctly | driver.rs:51,195,268 | pl011-minimized-datasheet.md:352-356 | Optional reset toggled on init. Req: pl011-spec-requirements.md:261 |
| REQ-RES-PIN-01 | captured-with-issue | driver.rs:25,55,202,213,284,292 | n/a | Pinctrl exists, but default/sleep not applied in port_open/port_close. Req: pl011-spec-requirements.md:264 |
| REQ-RES-IRQ-01 | captured-correctly | driver.rs:45,367 | n/a | IRQ resource plus ISR entry. Req: pl011-spec-requirements.md:267 |
| REQ-RES-MMIO-01 | captured-correctly | driver.rs:36; registers.rs:3 | pl011-minimized-datasheet.md:751-753 | MMIO register block declared. Req: pl011-spec-requirements.md:270 |
| REQ-INT-LIN-01 | deferred-to-backend-emission | n/a | n/a | Backend emission concern per prompt. Req: pl011-spec-requirements.md:277 |
| REQ-INT-ZEP-01 | deferred-to-backend-emission | n/a | n/a | Backend emission concern per prompt. Req: pl011-spec-requirements.md:284 |
| REQ-CORR-REGORDER-01 | captured-correctly | driver.rs:160,162 | pl011-minimized-datasheet.md:1018-1025 | Divisors before LCR_H. Req: pl011-spec-requirements.md:306 |
| REQ-CORR-REGORDER-02 | captured-correctly | driver.rs:120,122,160 | pl011-minimized-datasheet.md:1012,1018-1025 | LCR_H write includes FEN after divisors. Req: pl011-spec-requirements.md:310 |
| REQ-CORR-RESERVED-01 | not-captured | registers.rs:58,92,144,162,207,222 | pl011-minimized-datasheet.md:1009,1064,1092,1108,1180,1201 | No explicit reserved-bit policy metadata. Req: pl011-spec-requirements.md:313 |
| REQ-CORR-ICR-W1C | captured-correctly | driver.rs:87,92,253,259 | pl011-minimized-datasheet.md:1174 | Writes 1 only to clear bits. Req: pl011-spec-requirements.md:317 |
| REQ-CORR-MASK-SHADOW | not-captured | driver.rs:57,131,153 | n/a | No software IMSC shadow. Req: pl011-spec-requirements.md:320 |
| REQ-BC-RES-TABLE | captured-correctly | driver.rs:36,55 | n/a | Optional resources use `Option`. Req: pl011-spec-requirements.md:327 |
| REQ-BC-ISR-ENTRY | captured-correctly | driver.rs:367,370 | n/a | Stable `#[rseq::isr] handle_irq`. Req: pl011-spec-requirements.md:331 |
| REQ-BC-LIFECYCLE | captured-with-issue | driver.rs:263,301 | n/a | Lifecycle methods exist; resource ownership/precondition docs absent. Req: pl011-spec-requirements.md:335 |

**Q2 Register Descriptor Audit**

| register | field | datasheet-section | registers.rs-cite | issue |
|---|---|---|---|---|
| UARTDR | DATA, FE, PE, BE, OE | 3.3.1, pl011-minimized-datasheet.md:828-837 | registers.rs:7-13 | Shape/access match; reset/reserved 15:12 not encoded. |
| UARTRSR/UARTECR | FE, PE, BE, OE | 3.3.2, pl011-minimized-datasheet.md:848-859 | registers.rs:16-21 | Field shape matches; ECR “write any value clears” side effect not encoded. |
| UARTFR | CTS, DSR, DCD, BUSY, RXFE, TXFF, RXFF, TXFE, RI | 3.3.3, pl011-minimized-datasheet.md:866-881 | registers.rs:27-37 | Shape/access match; reset flags not encoded. |
| UARTILPR | ILPDVSR | 3.3.4, pl011-minimized-datasheet.md:883-904 | registers.rs:43-45 | Shape/access match; zero-illegal/reset not encoded. |
| UARTIBRD | BAUD_DIVINT | 3.3.5, pl011-minimized-datasheet.md:909-917 | registers.rs:48-50 | Shape/access match; invalid 0/max policy not encoded. |
| UARTFBRD | BAUD_DIVFRAC | 3.3.6, pl011-minimized-datasheet.md:919-940 | registers.rs:53-55 | Shape/access match; IBRD-dependent limits not encoded. |
| UARTLCR_H | BRK, PEN, EPS, STP2, FEN, WLEN, SPS | 3.3.7, pl011-minimized-datasheet.md:999-1045 | registers.rs:58-89 | Field shape/named values match; reserved 15:8 and LCR_H latch semantics not encoded. |
| UARTCR | UARTEN, SIREN, SIRLP, LBE, TXE, RXE, DTR, RTS, OUT1, OUT2, RTSEN, CTSEN | 3.3.8, pl011-minimized-datasheet.md:1047-1078 | registers.rs:92-141 | Shape/access match; reset `0x0300` and reserved 6:3 policy not encoded. |
| UARTIFLS | TXIFLSEL, RXIFLSEL | 3.3.9, pl011-minimized-datasheet.md:1080-1094 | registers.rs:144-159 | Shape/access match; reset `0x12` and reserved-bit policy not encoded. |
| UARTIMSC | RIMIM..OEIM | 3.3.10, pl011-minimized-datasheet.md:1096-1119 | registers.rs:162-174 | Shape/access match; reset/reserved policy not encoded. |
| UARTRIS | RIRMIS..OERIS | 3.3.11, pl011-minimized-datasheet.md:1121-1147 | registers.rs:177-189 | Shape/access match; modem reset undefined not encoded. |
| UARTMIS | RIMMIS..OEMIS | 3.3.12, pl011-minimized-datasheet.md:1149-1170 | registers.rs:192-204 | Shape/access match; modem reset undefined not encoded. |
| UARTICR | RIMIC..OEIC | 3.3.13, pl011-minimized-datasheet.md:1172-1191 | registers.rs:207-219 | Width/access match; W1C and reserved 15:11 policy not encoded in descriptor. |
| UARTDMACR | RXDMAE, TXDMAE, DMAONERR | 3.3.14, pl011-minimized-datasheet.md:1193-1204 | registers.rs:222-235 | Shape/access match; reset/reserved policy not encoded. |
| ID registers | PeriphID0-3, PCellID0-3 | 3.3.15-3.3.16, pl011-minimized-datasheet.md:1206-1366 | registers.rs:240-304 | Field values match r1p5; reset values not encoded as reset metadata. |
| reserved gaps | `_reserved0`, `_reserved1`, `_reserved2` | 3.2, pl011-minimized-datasheet.md:775-794 | registers.rs:23-24,39-40,237-238 | Address gaps match; test/future-ID regions are safely reserved, but not distinguished. |

**Q3 Behavioral Contracts**

- Baud divisor: captured with issue. `set_baud_rate` uses `(uart_clk_hz * 4) / baud_rate`, which truncates [driver.rs:105-108]. Datasheet requires fractional rounding with `+ 0.5` [pl011-minimized-datasheet.md:404-407]. IBRD/FBRD before LCR_H is correct [driver.rs:160-162], [pl011-minimized-datasheet.md:1018-1025].

- LCR_H latch: captured for normal configuration. LCR_H is written after divisor writes [driver.rs:120-123,160-162], matching the latch-on-LCR_H-write rule [pl011-minimized-datasheet.md:1018-1025]. RMW writes to LCR_H also occur for FIFO/break [driver.rs:75-77,353-363].

- ICR W1C: captured correctly. `clear_all_interrupts` and `clear_explicit_interrupts` write ones only for selected clear bits [driver.rs:87-92,253-259], matching W1C [pl011-minimized-datasheet.md:1174].

- IMSC mask shadow: not captured. The struct has only `pending_tx` as local mutable state [driver.rs:57], and IMSC hot paths use live RMW [driver.rs:131-153].

- Break ordering: captured with issue. `send_break` toggles `LCR_H.BRK` directly [driver.rs:358-363], but the datasheet says break starts after the current character, must last at least two frames, and FIFO integrity is not guaranteed after BRK [pl011-minimized-datasheet.md:1016,1043-1045].

- FIFO enable/disable: captured with issue. FEN is used [driver.rs:75-77,120-123,353-355], but TX flush toggles FEN without the datasheet’s disable, wait, flush, reprogram, enable sequence [pl011-minimized-datasheet.md:1072-1078].

- UART enable ordering: captured with issue. The fixture disables, programs divisors/LCR_H, then enables [driver.rs:156-168], but it does not wait for current TX/RX completion before reprogramming [pl011-minimized-datasheet.md:839-840,1040-1042]. TXE/RXE/UARTEN are set together, which satisfies the enable-bit dependency [pl011-minimized-datasheet.md:1069-1070].

- RX overrun: no mismatch found. Datasheet says FIFO contents are preserved and the new shift-register data is not written when full [pl011-minimized-datasheet.md:434,856]. Fixture only maps DR.OE to `SerialErrors::OVERRUN` [driver.rs:322-325], so it does not contradict that policy.

- Reserved-bit preservation: not fully captured. The relevant descriptors omit explicit reserved-bit policy [registers.rs:58-89,92-141,144-159,162-174,207-219,222-235], while the datasheet marks reserved bits as do-not-modify [pl011-minimized-datasheet.md:1009,1064,1092,1108,1180,1201]. ICR is not RMW’d; DMACR is unused.

**Q4 Status Snapshot Cross-Check**

The scheduled fixture work list is mostly accurate: REQ-CFG-03, REQ-IRQ-03, REQ-IRQ-04, REQ-CORR-RESERVED-01, and REQ-CORR-MASK-SHADOW are genuinely missing or incomplete [pl011-full-spec-status.md:47-66]. REQ-CFG-04 is partially present for the hard-coded 8N1 path, but remains incomplete once configurable line format is required [driver.rs:120-123,295-300].

The status file incorrectly lists REQ-INT-LIN-01 and REQ-INT-ZEP-01 as scheduled fixture work [pl011-full-spec-status.md:67-72]. Per the prompt and requirement wording, these are backend-emission concerns [pl011-spec-requirements.md:277-289].

Additional gaps not listed: divisor rounding [driver.rs:105-108], missing wait/flush sequencing before reconfiguration [driver.rs:156-168], break duration/TX-state handling [driver.rs:358-363], fallback clock as const rather than property [driver.rs:66,172-176], pinctrl not applied in port_open/port_close [driver.rs:284-292], single-byte TX ISR staging [driver.rs:57,237-245], and missing lifecycle precondition documentation [driver.rs:263-301].

**Q5 Datasheet-Unsupported Or Over-Specified Behavior**

Several fixture choices are legal software policy but not datasheet-derived hardware truth: default baud `115200` [driver.rs:39-40], default UART clock `48_000_000` [driver.rs:66], static IFLS choice RX half/TX eighth [driver.rs:95-99], single-byte `pending_tx` staging [driver.rs:57,237-245], and reset pulse duration of 10 us [driver.rs:195-198]. The datasheet only gives hardware capabilities and minimum reset constraints, not those policy values [pl011-minimized-datasheet.md:354,1082-1094].

`flush_rx_fifo` drains RX by repeated DR reads [driver.rs:347-350]. That is a reasonable software flush, but the datasheet describes receive FIFO readout rather than a dedicated RX flush command [pl011-minimized-datasheet.md:302,826]. `clear_all_interrupts` clears all 11 ICR bits including modem clears [driver.rs:87-92], which the datasheet permits [pl011-minimized-datasheet.md:1178-1191], but modem IRQ handling is outside the Stage-1 fixture claim.

**Q6 Minimal Fixes**

Priority order:

1. Add `imsc_shadow: u32` near [driver.rs:57], initialize it to 0, and replace `mask_all_interrupts`, `enable_rx_interrupts`, `enable_tx_interrupts`, `disable_tx_interrupts` [driver.rs:79-153] with shadow write-through helpers. Add error-mask enable/disable helpers for FE/PE/BE/OE. Covers REQ-IRQ-03, REQ-IRQ-04, REQ-CORR-MASK-SHADOW, REQ-PERF-LATENCY-01.

2. Fix divisor rounding at [driver.rs:105-108] and use `SerialConfig` in `configure` [driver.rs:295-300] to select baud, data bits, parity, and stop bits into LCR_H fields. Covers REQ-CFG-01, REQ-CFG-03, REQ-CFG-04.

3. Add a reconfiguration helper before divisor/LCR_H/CR changes: disable UART, wait for `FR.BUSY == 0`, flush TX via FEN, then reprogram and enable [driver.rs:68-77,156-168,353-355]. Covers REQ-LIFE-01, REQ-LIFE-02, REQ-FIFO-03.

4. Update `send_break` [driver.rs:358-363] to wait for TX empty and hold BRK for at least two frame times, or encode a backend delay obligation. Covers REQ-BRK-01.

5. Extend descriptors or metadata around [registers.rs:58-235] to encode reserved-bit policy for CR, LCR_H, IFLS, IMSC, ICR, and DMACR. Covers REQ-CORR-RESERVED-01.

6. Add an input-clock fallback property near [driver.rs:39-43], apply default pins in `port_open`, sleep pins in `port_close`, and document lifecycle resource ownership near [driver.rs:263-301]. Covers REQ-RES-CLK-01, REQ-RES-PIN-01, REQ-BC-LIFECYCLE.

7. Replace single-byte TX staging [driver.rs:57,237-245] with batch-neutral queue/service semantics or remove ISR wording that constrains TX to one byte. Covers REQ-PERF-BATCH-01.

**Q7 Final Verdict**

stage-1-gap-large

The fixture is not Stage-1 complete as-is. The minimal delta is more than five items because correctness gaps span configuration, interrupt masking, reserved-bit policy, lifecycle ordering, break generation, resource semantics, and batch-neutral TX behavior.

**RECOMMENDATION**

Implement the IMSC shadow/error-mask helpers first, then SerialConfig-driven line control with corrected baud rounding, then the safe reconfiguration sequence. Those three changes retire the highest-risk Stage-1 blockers. Follow with reserved-bit policy metadata, break timing, resource/lifecycle cleanup, and TX batching neutrality before declaring Stage-1 complete.
