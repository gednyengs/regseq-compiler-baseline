**Q1 Requirement Audit**

| REQ-ID | verdict | fixture-cite | delta-from-round-1 |
|---|---|---|---|
| REQ-TX-01 | captured-correctly | pl011-spec-requirements.md:92-94; driver.rs:489-490,518-520; registers.rs:7-13 | Still green; `tx_byte` is now the unchecked DR primitive. |
| REQ-TX-02 | captured-correctly | pl011-spec-requirements.md:95-97; driver.rs:469-490,518-520,581-588 | Improved; no single-byte `pending_tx` ISR contract remains. |
| REQ-RX-01 | captured-correctly | pl011-spec-requirements.md:98-100; driver.rs:493-499; registers.rs:7-13 | Still green. |
| REQ-RX-02 | captured-correctly | pl011-spec-requirements.md:101-103; driver.rs:351-357,577-579 | Still green. |
| REQ-RX-03 | captured-correctly | pl011-spec-requirements.md:104-107; driver.rs:501-512; registers.rs:8-12 | Still green. |
| REQ-RX-04 | captured-correctly | pl011-spec-requirements.md:108-111; driver.rs:498-515 | Still green; one DR read feeds local decode. |
| REQ-CFG-01 | captured-correctly | pl011-spec-requirements.md:115-118; driver.rs:40-51,145-155,305-310 | Fixed; divisor uses round-to-nearest. |
| REQ-CFG-02 | captured-correctly | pl011-spec-requirements.md:119-123; driver.rs:270-273,291-294 | Still green; configurable path now also ordered. |
| REQ-CFG-03 | captured-correctly | pl011-spec-requirements.md:124-126; driver.rs:172-198,461-465 | Fixed; `SerialConfig` now drives WLEN/parity/stop bits. |
| REQ-CFG-04 | captured-correctly | pl011-spec-requirements.md:127-129; driver.rs:270-277,291-294 | Fixed; divisor/LCR_H/enable order retained. |
| REQ-FIFO-01 | captured-correctly | pl011-spec-requirements.md:133-134; driver.rs:100,197,533-534; registers.rs:75-78 | Still green. |
| REQ-FIFO-02 | captured-correctly | pl011-spec-requirements.md:135-139; driver.rs:131-134; registers.rs:143-159 | Still green; static IFLS remains Stage 1. |
| REQ-FIFO-03 | captured-correctly | pl011-spec-requirements.md:140-143; driver.rs:526-535 | Still green as hook surface; lifecycle bring-up now has safe disable/wait/flush. |
| REQ-LIFE-01 | captured-correctly | pl011-spec-requirements.md:147-149; driver.rs:82-96,214-217,264-278,443-453 | Fixed; disable/wait ordering added. |
| REQ-LIFE-02 | captured-with-issue | pl011-spec-requirements.md:150-153; driver.rs:286-295,172-198 | Valid configs follow safe order; unsupported configs can return after disable and before re-enable. New small issue. |
| REQ-IRQ-01 | captured-correctly | pl011-spec-requirements.md:167-170; driver.rs:108-128,360-366; registers.rs:161-219 | Still green. |
| REQ-IRQ-02 | captured-correctly | pl011-spec-requirements.md:171-173; driver.rs:567-599 | Still green; RX/TX/error clear paths present. |
| REQ-IRQ-03 | captured-correctly | pl011-spec-requirements.md:174-177; driver.rs:65-72,108-118,221-245 | Fixed; IMSC shadow replaces live RMW. |
| REQ-IRQ-04 | captured-correctly | pl011-spec-requirements.md:178-184; driver.rs:76-80,221-251; registers.rs:167-173 | Fixed; RX/RT/TX/error masks are individually manipulable. |
| REQ-BRK-01 | captured-correctly | pl011-spec-requirements.md:188-190; driver.rs:537-562; registers.rs:59-62 | Fixed by documentation; BRK toggle captured, duration delegated. |
| REQ-PERF-BATCH-01 | captured-correctly | pl011-spec-requirements.md:213-218; driver.rs:469-490,581-588 | Fixed; no byte-at-a-time ISR staging. |
| REQ-PERF-LATENCY-01 | captured-correctly | pl011-spec-requirements.md:234-237; driver.rs:108-118,221-245,489-490 | Fixed; hot IMSC RMW removed. |
| REQ-ID-01 | captured-correctly | pl011-spec-requirements.md:243-245; driver.rs:34 | Still green. |
| REQ-ID-02 | captured-correctly | pl011-spec-requirements.md:246-250; driver.rs:34 | Still green. |
| REQ-RES-CLK-01 | captured-correctly | pl011-spec-requirements.md:254-256; driver.rs:46-51,305-310 | Fixed; fallback clock is now a property. |
| REQ-RES-CLK-02 | captured-correctly | pl011-spec-requirements.md:257-260; driver.rs:312-325,436-439,450-454 | Still green for current open/close path. |
| REQ-RES-RST-01 | captured-correctly | pl011-spec-requirements.md:261-263; driver.rs:59-60,327-332,415 | Still green. |
| REQ-RES-PIN-01 | captured-correctly | pl011-spec-requirements.md:264-266; driver.rs:26-32,62-63,335-348,436,458 | Fixed; default/sleep states are applied. |
| REQ-RES-IRQ-01 | captured-correctly | pl011-spec-requirements.md:267-269; driver.rs:53-54,567-599 | Still green. |
| REQ-RES-MMIO-01 | captured-correctly | pl011-spec-requirements.md:270-273; driver.rs:37-38; registers.rs:3-5 | Still green. |
| REQ-INT-LIN-01 | deferred-to-backend-emission | pl011-spec-requirements.md:277-283; pl011-full-spec-status.md:76-86 | Correctly backend-level, not fixture-level. |
| REQ-INT-ZEP-01 | deferred-to-backend-emission | pl011-spec-requirements.md:284-289; pl011-full-spec-status.md:76-90 | Correctly backend-level, not fixture-level. |
| REQ-CORR-REGORDER-01 | captured-correctly | pl011-spec-requirements.md:306-309; driver.rs:270-273,291-294 | Still green for valid reprogramming paths. |
| REQ-CORR-REGORDER-02 | captured-correctly | pl011-spec-requirements.md:310-312; driver.rs:270-273,291-294 | Still green. |
| REQ-CORR-RESERVED-01 | captured-correctly | pl011-spec-requirements.md:313-316; driver.rs:108-112,124-128,196-197; call_lowering.rs:47-65 | Fixed/documented as captured by framework lowering. |
| REQ-CORR-ICR-W1C | captured-correctly | pl011-spec-requirements.md:317-319; driver.rs:124-128,360-366; registers.rs:207-219 | Still green. |
| REQ-CORR-MASK-SHADOW | captured-correctly | pl011-spec-requirements.md:320-323; driver.rs:65-72,108-118,221-245 | Fixed. |
| REQ-BC-RES-TABLE | captured-correctly | pl011-spec-requirements.md:327-330; driver.rs:37-63 | Still green. |
| REQ-BC-ISR-ENTRY | captured-correctly | pl011-spec-requirements.md:331-334; driver.rs:567-599 | Still green. |
| REQ-BC-LIFECYCLE | captured-correctly | pl011-spec-requirements.md:335-338; driver.rs:370-407,407-429,431-467 | Fixed; resource ownership docs added. |

**Q2 Register Descriptor Audit**

| register | verdict | descriptor-cite | datasheet-cite | note |
|---|---|---|---|---|
| UARTDR | captured-correctly | registers.rs:7-13 | pl011-minimized-datasheet.md:828-837 | DATA/error bit shape matches. |
| UARTRSR/UARTECR | captured-correctly | registers.rs:15-21 | pl011-minimized-datasheet.md:842-859 | Field shape matches; ECR side effect is behavioral, not descriptor metadata. |
| UARTFR | captured-correctly | registers.rs:26-37 | pl011-minimized-datasheet.md:864-881 | Flag bits match. |
| UARTILPR | captured-correctly | registers.rs:42-45 | pl011-minimized-datasheet.md:883-904 | Descriptor present; IrDA is deferred. |
| UARTIBRD | captured-correctly | registers.rs:47-50 | pl011-minimized-datasheet.md:909-917 | Width matches. |
| UARTFBRD | captured-correctly | registers.rs:52-55 | pl011-minimized-datasheet.md:919-940 | Width matches. |
| UARTLCR_H | captured-correctly | registers.rs:57-89 | pl011-minimized-datasheet.md:999-1045 | Fields match; reserved handling relies on write/modify lowering. |
| UARTCR | captured-correctly | registers.rs:91-141 | pl011-minimized-datasheet.md:1047-1078 | Fields match; Stage 2 must use `modify` for RTS/DTR preservation. |
| UARTIFLS | captured-correctly | registers.rs:143-159 | pl011-minimized-datasheet.md:1080-1094 | Trigger enums match valid encodings. |
| UARTIMSC | captured-correctly | registers.rs:161-174 | pl011-minimized-datasheet.md:1096-1119 | All Stage 1 masks modeled. |
| UARTRIS | captured-correctly | registers.rs:176-189 | pl011-minimized-datasheet.md:1121-1147 | Raw status bits modeled. |
| UARTMIS | captured-correctly | registers.rs:191-204 | pl011-minimized-datasheet.md:1149-1170 | Masked status bits modeled. |
| UARTICR | captured-correctly | registers.rs:206-219 | pl011-minimized-datasheet.md:1172-1191 | WO clear bits modeled; W1C honored in driver.rs:124-128,360-366. |
| UARTDMACR | captured-correctly | registers.rs:221-235 | pl011-minimized-datasheet.md:1193-1204 | Descriptor present; DMA is Stage 3. |
| ID registers | captured-correctly | registers.rs:240-304 | pl011-minimized-datasheet.md:1206-1366 | r1p5-compatible ID values modeled. |
| reserved gaps | captured-correctly | registers.rs:23-24,39-40,237-238 | pl011-minimized-datasheet.md:751-758,775-794 | Address gaps match. |

Reserved-bit conclusion: confirm for Stage 1 behavior, with wording caveat. `write_field` lowers to `Stmt::RegWrite` and `modify` lowers to `Stmt::RegModify` in call_lowering.rs:47-65. Current Stage 1 writes either write zero into unmasked bits or use RMW preservation. The descriptors themselves still do not carry reserved-bit policy metadata, so the status doc should call this “captured by framework lowering,” not “encoded in descriptors.”

**Q3 Behavioral Contracts**

- Baud divisor: captured-correctly. `set_baud_rate` uses `(clk * 8 + baud) / (baud * 2)` at driver.rs:145-155, matching the datasheet rounding rule at pl011-minimized-datasheet.md:404-407 and Example 3-1 at pl011-minimized-datasheet.md:944-955. It reproduces Table 3-9 values at pl011-minimized-datasheet.md:985-997.

- Divisor/LCR_H latch order: captured-correctly for valid configs. `bring_up_port` writes IBRD/FBRD before LCR_H at driver.rs:270-273, and `reprogram_line_format` does the same at driver.rs:291-294, matching pl011-minimized-datasheet.md:1018-1025.

- Disable/wait/re-enable sequence: captured-with-small-issue. Valid reconfigure disables and waits at driver.rs:286-289, then reprograms and enables at driver.rs:291-294, consistent with pl011-minimized-datasheet.md:839-840 and pl011-minimized-datasheet.md:1040-1042. The issue is error handling: unsupported configs can return from `configure_line_control` at driver.rs:178 or driver.rs:193 after UART disable at driver.rs:288 and before re-enable at driver.rs:294.

- IMSC shadow: captured-correctly. Shadow storage is driver.rs:65-72, full-mask write-through is driver.rs:108-112, and RX/TX/error helpers mutate the shadow before committing at driver.rs:221-251. Reconfigure preserves the shadow at driver.rs:282-295.

- ICR W1C: captured-correctly. Full clear and explicit clear write ones only to selected bits at driver.rs:124-128 and driver.rs:360-366, matching pl011-minimized-datasheet.md:1172-1191.

- Break: captured-correctly. `send_break` toggles `LCR_H.BRK` at driver.rs:557-562; the datasheet duration obligation is documented as caller-owned at driver.rs:546-551, matching pl011-minimized-datasheet.md:1016.

**Q4 Status Cross-Check**

The new split exists and the backend-emission section is accurate: REQ-INT-LIN-01 and REQ-INT-ZEP-01 are backend-level at pl011-full-spec-status.md:76-90 and match pl011-spec-requirements.md:277-289.

The fixture-work section is stale. It still says REQ-CFG-03/04 are scheduled and hard-coded at pl011-full-spec-status.md:48-53, but the fixture maps `SerialConfig` at driver.rs:172-198 and calls it from `configure` at driver.rs:461-465. It still says REQ-IRQ-03 lacks a shadow at pl011-full-spec-status.md:54-57, but the shadow exists at driver.rs:65-72 and commits at driver.rs:108-118. It still says REQ-IRQ-04 has no error enable path at pl011-full-spec-status.md:58-62, but helpers exist at driver.rs:238-251. REQ-CORR-MASK-SHADOW at pl011-full-spec-status.md:73-74 is also now implemented.

REQ-CORR-RESERVED-01 at pl011-full-spec-status.md:63-72 is substantively correct, but should be moved out of “Scheduled fixture work” because it is documented as captured by framework lowering.

**Q5 New Datasheet-Unsupported Behavior**

I found one new software behavior issue, not a direct datasheet-unsupported register behavior: `reprogram_line_format` disables the UART at driver.rs:288, writes divisors at driver.rs:291-292, then can return `Unsupported` from `configure_line_control` at driver.rs:178 or driver.rs:193 before the re-enable at driver.rs:294. That can leave an open port disabled after a rejected config.

`enable_error_interrupts` being opt-in is acceptable for Stage 1: REQ-IRQ-04 requires named IRQ-source manipulation at pl011-spec-requirements.md:178-180, and the helpers provide that at driver.rs:238-251. No Stage 1 requirement says bring-up must enable FEIM/PEIM/BEIM/OEIM by default.

`default_serial_config` hard-coding 8N1 is acceptable for default open: it uses `current_speed` at driver.rs:204-211, while REQ-CFG-01 only requires baud plus input-clock property at pl011-spec-requirements.md:115-118. Explicit line format remains configurable through `Serial::configure` at driver.rs:461-465.

**Q6 Minimal Fixes**

Remaining `captured-with-issue`: REQ-LIFE-02.

Minimal fix: validate the `SerialConfig` before disabling or programming the UART in `reprogram_line_format` at driver.rs:286-295. The cleanest small patch is to split `configure_line_control` into a pure `line_control_value(config) -> Result<FieldValue<_, lcr_h::Register>, RegseqError>` plus the existing write wrapper. `reprogram_line_format` can call the pure validator first, then disable/wait, write divisors, write the precomputed LCR_H value, and re-enable. This preserves the current valid-config order while making unsupported `DataBits::Nine` at driver.rs:178 and non-`None` flow control at driver.rs:192-193 fail without side effects.

Documentation fix: update pl011-full-spec-status.md:48-74 so fixed items move from “Scheduled fixture work” to “Already captured,” and keep REQ-CORR-RESERVED-01 as a captured-by-framework note rather than scheduled fixture work.

Verification note: standalone `cargo check` currently fails because the fixture depends on `regseq` at Cargo.toml:10 but uses `rseq::...` attributes at driver.rs:26, driver.rs:34, driver.rs:567, and registers.rs:3. I did not score that against the PL011 REQ table because it is outside the stated sources of truth, but it blocks standalone compilation.

**Q7 Final Verdict**

stage-1-gap-small

The Round 1 semantic fixes mostly landed. The remaining paper-min fixture issue is the rejected-config side effect in `reprogram_line_format`; it should be fixed in Round 3 because it is small and local, but it does not block valid Stage 1 core comparison workloads. The status doc is also stale and should be updated before declaring closure. The standalone `cargo check` alias issue should be resolved if this fixture is expected to compile directly.

CONVERGENCE SIGNAL more rounds expected
