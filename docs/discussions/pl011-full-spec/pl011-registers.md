# Chapter 3 Programmers Model

This chapter describes the memory map and registers of the UART. It contains the following sections:

- About the programmers model on page 3-2
- Summary of registers on page 3-3
- Register descriptions on page 3-5.

---

## 3.1 About the programmers model

The following information applies to the UART registers:

- The base address of the UART is not fixed, and can be different for any particular system implementation. The offset of each register from the base address is fixed.

- Do not attempt to access reserved or unused address locations. Attempting to access these location can result in Unpredictable behavior of the UART.

- Unless otherwise stated in the accompanying text:
  - do not modify undefined register bits
  - ignore undefined register bits on reads
  - all register bits are reset to a logic 0 by a system or power-on reset.

- The Type column in Table 3-1 on page 3-3 describes the access types as follows:
  - **RW** - Read and write.
  - **RO** - Read only.
  - **WO** - Write only.

---

## 3.2 Summary of registers

**Table 3-1 UART register summary**

| Offset | Name | Type | Reset | Width | Description |
|--------|------|------|-------|-------|-------------|
| 0x000 | UARTDR | RW | 0x--- | 12/8 | Data Register, UARTDR on page 3-5 |
| 0x004 | UARTRSR/UARTECR | RW | 0x0 | 4/0 | Receive Status Register/Error Clear Register, UARTRSR/UARTECR on page 3-6 |
| 0x008-0x014 | - | - | - | - | Reserved |
| 0x018 | UARTFR | RO | 0b-10010--- | 9 | Flag Register, UARTFR on page 3-8 |
| 0x01C | - | - | - | - | Reserved |
| 0x020 | UARTILPR | RW | 0x00 | 8 | IrDA Low-Power Counter Register, UARTILPR on page 3-9 |
| 0x024 | UARTIBRD | RW | 0x0000 | 16 | Integer Baud Rate Register, UARTIBRD on page 3-9 |
| 0x028 | UARTFBRD | RW | 0x00 | 6 | Fractional Baud Rate Register, UARTFBRD on page 3-10 |
| 0x02C | UARTLCR_H | RW | 0x00 | 8 | Line Control Register, UARTLCR_H on page 3-12 |
| 0x030 | UARTCR | RW | 0x0300 | 16 | Control Register, UARTCR on page 3-15 |
| 0x034 | UARTIFLS | RW | 0x12 | 6 | Interrupt FIFO Level Select Register, UARTIFLS on page 3-17 |
| 0x038 | UARTIMSC | RW | 0x000 | 11 | Interrupt Mask Set/Clear Register, UARTIMSC on page 3-17 |
| 0x03C | UARTRIS | RO | 0x00- | 11 | Raw Interrupt Status Register, UARTRIS on page 3-19 |
| 0x040 | UARTMIS | RO | 0x00- | 11 | Masked Interrupt Status Register, UARTMIS on page 3-20 |
| 0x044 | UARTICR | WO | - | 11 | Interrupt Clear Register, UARTICR on page 3-21 |
| 0x048 | UARTDMACR | RW | 0x00 | 3 | DMA Control Register, UARTDMACR on page 3-22 |
| 0x04C-0x07C | - | - | - | - | Reserved |
| 0x080-0x08C | - | - | - | - | Reserved for test purposes |
| 0x090-0xFCC | - | - | - | - | Reserved |
| 0xFD0-0xFDC | - | - | - | - | Reserved for future ID expansion |
| 0xFE0 | UARTPeriphID0 | RO | 0x11 | 8 | UARTPeriphID0 Register on page 3-23 |
| 0xFE4 | UARTPeriphID1 | RO | 0x10 | 8 | UARTPeriphID1 Register on page 3-24 |
| 0xFE8 | UARTPeriphID2 | RO | 0x_4 (a) | 8 | UARTPeriphID2 Register on page 3-24 |
| 0xFEC | UARTPeriphID3 | RO | 0x00 | 8 | UARTPeriphID3 Register on page 3-25 |
| 0xFF0 | UARTPCellID0 | RO | 0x0D | 8 | UARTPCellID0 Register on page 3-26 |
| 0xFF4 | UARTPCellID1 | RO | 0xF0 | 8 | UARTPCellID1 Register on page 3-26 |
| 0xFF8 | UARTPCellID2 | RO | 0x05 | 8 | UARTPCellID2 Register on page 3-26 |
| 0xFFC | UARTPCellID3 | RO | 0xB1 | 8 | UARTPCellID3 Register on page 3-27 |

(a) The value depends on the revision of the UART. See Table 3-21 on page 3-24.
# Programmers Model

## 3.3 Register descriptions

This section describes the UART registers. The test registers are described in Chapter 4 Programmers Model for Test. Table 3-1 on page 3-3 lists the cross references to individual registers.

### 3.3.1 Data Register, UARTDR

The UARTDR Register is the data register.

For words to be transmitted:

- if the FIFOs are enabled, data written to this location is pushed onto the transmit FIFO
- if the FIFOs are not enabled, data is stored in the transmitter holding register (the bottom word of the transmit FIFO).

The write operation initiates transmission from the UART. The data is prefixed with a start bit, appended with the appropriate parity bit (if parity is enabled), and a stop bit. The resultant word is then transmitted.

For received words:

- if the FIFOs are enabled, the data byte and the 4-bit status (break, frame, parity, and overrun) is pushed onto the 12-bit wide receive FIFO
- if the FIFOs are not enabled, the data byte and status are stored in the receiving holding register (the bottom word of the receive FIFO).

The received data byte is read by performing reads from the UARTDR Register along with the corresponding status information. The status information can also be read by a read of the UARTRSR/UARTECR Register as shown in Table 3-3 on page 3-7.

**Note**

You must disable the UART before any of the control registers are reprogrammed. When the UART is disabled in the middle of transmission or reception, it completes the current character before stopping.

**Table 3-2 UARTDR Register**

| Bits | Name | Function |
|------|------|----------|
| 15:12 | - | Reserved. |
| 11 | OE | Overrun error. This bit is set to 1 if data is received and the receive FIFO is already full. This is cleared to 0 once there is an empty space in the FIFO and a new character can be written to it. |
| 10 | BE | Break error. This bit is set to 1 if a break condition was detected, indicating that the received data input was held LOW for longer than a full-word transmission time (defined as start, data, parity and stop bits). In FIFO mode, this error is associated with the character at the top of the FIFO. When a break occurs, only one 0 character is loaded into the FIFO. The next character is only enabled after the receive data input goes to a 1 (marking state), and the next valid start bit is received. |
| 9 | PE | Parity error. When set to 1, it indicates that the parity of the received data character does not match the parity that the EPS and SPS bits in the Line Control Register, UARTLCR_H on page 3-12 select. In FIFO mode, this error is associated with the character at the top of the FIFO. |
| 8 | FE | Framing error. When set to 1, it indicates that the received character did not have a valid stop bit (a valid stop bit is 1). In FIFO mode, this error is associated with the character at the top of the FIFO. |
| 7:0 | DATA | Receive (read) data character. Transmit (write) data character. |

### 3.3.2 Receive Status Register/Error Clear Register, UARTRSR/UARTECR

The UARTRSR/UARTECR Register is the receive status register/error clear register.

Receive status can also be read from the UARTRSR Register. If the status is read from this register, then the status information for break, framing and parity corresponds to the data character read from the Data Register, UARTDR on page 3-5 prior to reading the UARTRSR Register. The status information for overrun is set immediately when an overrun condition occurs.

A write to the UARTECR Register clears the framing, parity, break, and overrun errors. All the bits are cleared to 0 on reset. Table 3-3 lists the bit assignment of the UARTRSR/UARTECR Register.

**Table 3-3 UARTRSR/UARTECR Register**

| Bits | Name | Function |
|------|------|----------|
| 7:0 | - | A write to this register clears the framing, parity, break, and overrun errors. The data value is not important. |
| 7:4 | - | Reserved, unpredictable when read. |
| 3 | OE | Overrun error. This bit is set to 1 if data is received and the FIFO is already full. This bit is cleared to 0 by a write to UARTECR. The FIFO contents remain valid because no more data is written when the FIFO is full, only the contents of the shift register are overwritten. The CPU must now read the data, to empty the FIFO. |
| 2 | BE | Break error. This bit is set to 1 if a break condition was detected, indicating that the received data input was held LOW for longer than a full-word transmission time (defined as start, data, parity, and stop bits). This bit is cleared to 0 after a write to UARTECR. In FIFO mode, this error is associated with the character at the top of the FIFO. When a break occurs, only one 0 character is loaded into the FIFO. The next character is only enabled after the receive data input goes to a 1 (marking state) and the next valid start bit is received. |
| 1 | PE | Parity error. When set to 1, it indicates that the parity of the received data character does not match the parity that the EPS and SPS bits in the Line Control Register, UARTLCR_H on page 3-12 select. This bit is cleared to 0 by a write to UARTECR. In FIFO mode, this error is associated with the character at the top of the FIFO. |
| 0 | FE | Framing error. When set to 1, it indicates that the received character did not have a valid stop bit (a valid stop bit is 1). This bit is cleared to 0 by a write to UARTECR. In FIFO mode, this error is associated with the character at the top of the FIFO. |

**Note**

The received data character must be read first from the Data Register, UARTDR on page 3-5 before reading the error status associated with that data character from the UARTRSR Register. This read sequence cannot be reversed, because the UARTRSR Register is updated only when a read occurs from the UARTDR Register. However, the status information can also be obtained by reading the UARTDR Register.

### 3.3.3 Flag Register, UARTFR

The UARTFR Register is the flag register. After reset TXFF, RXFF, and BUSY are 0, and TXFE and RXFE are 1. Table 3-4 lists the register bit assignments.

**Table 3-4 UARTFR Register**

| Bits | Name | Function |
|------|------|----------|
| 15:9 | - | Reserved, do not modify, read as zero. |
| 8 | RI | Ring indicator. This bit is the complement of the UART ring indicator, nUARTRI, modem status input. That is, the bit is 1 when nUARTRI is LOW. |
| 7 | TXFE | Transmit FIFO empty. The meaning of this bit depends on the state of the FEN bit in the Line Control Register, UARTLCR_H on page 3-12. If the FIFO is disabled, this bit is set when the transmit holding register is empty. If the FIFO is enabled, the TXFE bit is set when the transmit FIFO is empty. This bit does not indicate if there is data in the transmit shift register. |
| 6 | RXFF | Receive FIFO full. The meaning of this bit depends on the state of the FEN bit in the UARTLCR_H Register. If the FIFO is disabled, this bit is set when the receive holding register is full. If the FIFO is enabled, the RXFF bit is set when the receive FIFO is full. |
| 5 | TXFF | Transmit FIFO full. The meaning of this bit depends on the state of the FEN bit in the UARTLCR_H Register. If the FIFO is disabled, this bit is set when the transmit holding register is full. If the FIFO is enabled, the TXFF bit is set when the transmit FIFO is full. |
| 4 | RXFE | Receive FIFO empty. The meaning of this bit depends on the state of the FEN bit in the UARTLCR_H Register. If the FIFO is disabled, this bit is set when the receive holding register is empty. If the FIFO is enabled, the RXFE bit is set when the receive FIFO is empty. |
| 3 | BUSY | UART busy. If this bit is set to 1, the UART is busy transmitting data. This bit remains set until the complete byte, including all the stop bits, has been sent from the shift register. This bit is set as soon as the transmit FIFO becomes non-empty, regardless of whether the UART is enabled or not. |
| 2 | DCD | Data carrier detect. This bit is the complement of the UART data carrier detect, nUARTDCD, modem status input. That is, the bit is 1 when nUARTDCD is LOW. |
| 1 | DSR | Data set ready. This bit is the complement of the UART data set ready, nUARTDSR, modem status input. That is, the bit is 1 when nUARTDSR is LOW. |
| 0 | CTS | Clear to send. This bit is the complement of the UART clear to send, nUARTCTS, modem status input. That is, the bit is 1 when nUARTCTS is LOW. |

### 3.3.4 IrDA Low-Power Counter Register, UARTILPR

The UARTILPR Register is the IrDA low-power counter register. This is an 8-bit read/write register that stores the low-power counter divisor value used to generate the IrLPBaud16 signal by dividing down of UARTCLK. Table 3-5 lists the register bit assignments.

**Table 3-5 UARTILPR Register**

| Bits | Name | Function |
|------|------|----------|
| 7:0 | ILPDVSR | 8-bit low-power divisor value. These bits are cleared to 0 at reset. |

**Note**

Zero is an illegal value. Programming a zero value results in no IrLPBaud16 pulses being generated.

The IrLPBaud16 signal is generated by dividing down the UARTCLK signal according to the low-power divisor value written to the UARTILPR Register.

The low-power divisor value is calculated as follows:

- low-power divisor (ILPDVSR) = (F_UARTCLK / F_IrLPBaud16)

where F_IrLPBaud16 is nominally 1.8432MHz.

You must select the divisor so that 1.42MHz < F_IrLPBaud16 < 2.12MHz, results in a low-power pulse duration of 1.41-2.11us (three times the period of IrLPBaud16).

**Note**

In low-power IrDA mode the UART rejects random noise on the received serial data input by ignoring SIRIN pulses that are less than 3 periods of IrLPBaud16.

### 3.3.5 Integer Baud Rate Register, UARTIBRD

The UARTIBRD Register is the integer part of the baud rate divisor value. Table 3-6 lists the register bit assignments.

**Table 3-6 UARTIBRD Register**

| Bits | Name | Function |
|------|------|----------|
| 15:0 | BAUD DIVINT | The integer baud rate divisor. These bits are cleared to 0 on reset. |

### 3.3.6 Fractional Baud Rate Register, UARTFBRD

The UARTFBRD Register is the fractional part of the baud rate divisor value. Table 3-7 lists the register bit assignments.

**Table 3-7 UARTFBRD Register**

| Bits | Name | Function |
|------|------|----------|
| 5:0 | BAUD DIVFRAC | The fractional baud rate divisor. These bits are cleared to 0 on reset. |

The baud rate divisor is calculated as follows:

- Baud rate divisor BAUDDIV = (F_UARTCLK/(16 x Baud rate))

where F_UARTCLK is the UART reference clock frequency.

The BAUDDIV is comprised of the integer value (BAUD DIVINT) and the fractional value (BAUD DIVFRAC).

**Note**

- The contents of the UARTIBRD and UARTFBRD registers are not updated until transmission or reception of the current character is complete.
- The minimum divide ratio possible is 1 and the maximum is 65535(2^16 - 1). That is, UARTIBRD = 0 is invalid and UARTFBRD is ignored when this is the case.
- Similarly, when UARTIBRD = 65535 (that is 0xFFFF), then UARTFBRD must not be greater than zero. If this is exceeded it results in an aborted transmission or reception.

**Example 3-1 Calculating the divisor value**

If the required baud rate is 230400 and UARTCLK = 4MHz then:

Baud Rate Divisor = (4x10^6)/(16x230400) = 1.085

This means BRDI = 1 and BRDF = 0.085.

Therefore, fractional part, m = integer((0.085x64)+0.5) = 5

Generated baud rate divider = 1+5/64 = 1.078

Generated baud rate = (4x10^6)/(16x1.078) = 231911

Error = (231911-230400)/230400 x 100 = 0.656%

The maximum error using a 6-bit UARTFBRD Register = 1/64x100 = 1.56%. This occurs when m = 1, and the error is cumulative over 64 clock ticks.

**Table 3-8 Typical baud rates and integer divisors when UARTCLK=7.3728MHz**

| Programmed integer divisor | Bit rate (bps) |
|---------------------------|----------------|
| 0x1 | 460800 |
| 0x2 | 230400 |
| 0x4 | 115200 |
| 0x6 | 76800 |
| 0x8 | 57600 |
| 0xC | 38400 |
| 0x18 | 19200 |
| 0x20 | 14400 |
| 0x30 | 9600 |
| 0xC0 | 2400 |
| 0x180 | 1200 |
| 0x105D | 110 |

**Table 3-9 Integer and fractional divisors for typical baud rates when UARTCLK=4MHz**

| Programmed integer divisor | Programmed fractional divisor | Required bit rate (bps) | Generated bit rate (bps) | Error % |
|---------------------------|------------------------------|------------------------|-------------------------|---------|
| 0x1 | 0x5 | 230400 | 231911 | 0.656 |
| 0x2 | 0xB | 115200 | 115101 | 0.086 |
| 0x3 | 0x10 | 76800 | 76923 | 0.160 |
| 0x6 | 0x21 | 38400 | 38369 | 0.081 |
| 0x11 | 0x17 | 14400 | 14401 | 0.007 |
| 0x68 | 0xB | 2400 | 2400 | ~0 |
| 0x8E0 | 0x2F | 110 | 110 | ~0 |

### 3.3.7 Line Control Register, UARTLCR_H

The UARTLCR_H Register is the line control register. This register accesses bits 29 to 22 of the UART Line Control Register, UARTLCR.

All the bits are cleared to 0 when reset. Table 3-10 lists the register bit assignments.

**Table 3-10 UARTLCR_H Register**

| Bits | Name | Function |
|------|------|----------|
| 15:8 | - | Reserved, do not modify, read as zero. |
| 7 | SPS | Stick parity select. 0 = stick parity is disabled. 1 = either: if the EPS bit is 0 then the parity bit is transmitted and checked as a 1; if the EPS bit is 1 then the parity bit is transmitted and checked as a 0. This bit has no effect when the PEN bit disables parity checking and generation. See Table 3-11 on page 3-14 for the parity truth table. |
| 6:5 | WLEN | Word length. These bits indicate the number of data bits transmitted or received in a frame as follows: b11 = 8 bits, b10 = 7 bits, b01 = 6 bits, b00 = 5 bits. |
| 4 | FEN | Enable FIFOs: 0 = FIFOs are disabled (character mode) that is, the FIFOs become 1-byte-deep holding registers. 1 = transmit and receive FIFO buffers are enabled (FIFO mode). |
| 3 | STP2 | Two stop bits select. If this bit is set to 1, two stop bits are transmitted at the end of the frame. The receive logic does not check for two stop bits being received. |
| 2 | EPS | Even parity select. Controls the type of parity the UART uses during transmission and reception: 0 = odd parity. The UART generates or checks for an odd number of 1s in the data and parity bits. 1 = even parity. The UART generates or checks for an even number of 1s in the data and parity bits. This bit has no effect when the PEN bit disables parity checking and generation. See Table 3-11 on page 3-14 for the parity truth table. |
| 1 | PEN | Parity enable: 0 = parity is disabled and no parity bit added to the data frame. 1 = parity checking and generation is enabled. See Table 3-11 on page 3-14 for the parity truth table. |
| 0 | BRK | Send break. If this bit is set to 1, a low-level is continually output on the UARTTXD output, after completing transmission of the current character. For the proper execution of the break command, the software must set this bit for at least two complete frames. For normal use, this bit must be cleared to 0. |

The UARTLCR_H, UARTIBRD, and UARTFBRD registers form the single 30-bit wide UARTLCR Register that is updated on a single write strobe generated by a UARTLCR_H write. So, to internally update the contents of UARTIBRD or UARTFBRD, a UARTLCR_H write must always be performed at the end.

**Note**

- To update the three registers there are two possible sequences:
  - UARTIBRD write, UARTFBRD write, and UARTLCR_H write
  - UARTFBRD write, UARTIBRD write, and UARTLCR_H write.
- To update UARTIBRD or UARTFBRD only:
  - UARTIBRD write, or UARTFBRD write, and UARTLCR_H write.

**Table 3-11 Parity truth table**

| PEN | EPS | SPS | Parity bit (transmitted or checked) |
|-----|-----|-----|-------------------------------------|
| 0 | x | x | Not transmitted or checked |
| 1 | 1 | 0 | Even parity |
| 1 | 0 | 0 | Odd parity |
| 1 | 0 | 1 | 1 |
| 1 | 1 | 1 | 0 |

**Note**

- The UARTLCR_H, UARTIBRD, and UARTFBRD registers must not be changed:
  - when the UART is enabled
  - when completing a transmission or a reception when it has been programmed to become disabled.
- The FIFO integrity is not guaranteed under the following conditions:
  - after the BRK bit has been initiated
  - if the software disables the UART in the middle of a transmission with data in the FIFO, and then re-enables it.

### 3.3.8 Control Register, UARTCR

The UARTCR Register is the control register. All the bits are cleared to 0 on reset except for bits 9 and 8 that are set to 1. Table 3-12 lists the register bit assignments.

**Table 3-12 UARTCR Register**

| Bits | Name | Function |
|------|------|----------|
| 15 | CTSEn | CTS hardware flow control enable. If this bit is set to 1, CTS hardware flow control is enabled. Data is only transmitted when the nUARTCTS signal is asserted. |
| 14 | RTSEn | RTS hardware flow control enable. If this bit is set to 1, RTS hardware flow control is enabled. Data is only requested when there is space in the receive FIFO for it to be received. |
| 13 | Out2 | This bit is the complement of the UART Out2 (nUARTOut2) modem status output. That is, when the bit is programmed to a 1, the output is 0. For DTE this can be used as Ring Indicator (RI). |
| 12 | Out1 | This bit is the complement of the UART Out1 (nUARTOut1) modem status output. That is, when the bit is programmed to a 1 the output is 0. For DTE this can be used as Data Carrier Detect (DCD). |
| 11 | RTS | Request to send. This bit is the complement of the UART request to send, nUARTRTS, modem status output. That is, when the bit is programmed to a 1 then nUARTRTS is LOW. |
| 10 | DTR | Data transmit ready. This bit is the complement of the UART data transmit ready, nUARTDTR, modem status output. That is, when the bit is programmed to a 1 then nUARTDTR is LOW. |
| 9 | RXE | Receive enable. If this bit is set to 1, the receive section of the UART is enabled. Data reception occurs for either UART signals or SIR signals depending on the setting of the SIREN bit. When the UART is disabled in the middle of reception, it completes the current character before stopping. |
| 8 | TXE | Transmit enable. If this bit is set to 1, the transmit section of the UART is enabled. Data transmission occurs for either UART signals, or SIR signals depending on the setting of the SIREN bit. When the UART is disabled in the middle of transmission, it completes the current character before stopping. |
| 7 | LBE | Loopback enable. If this bit is set to 1 and the SIREN bit is set to 1 and the SIRTEST bit in the Test Control Register, UARTTCR on page 4-5 is set to 1, then the nSIROUT path is inverted, and fed through to the SIRIN path. The SIRTEST bit in the test register must be set to 1 to override the normal half-duplex SIR operation. This must be the requirement for accessing the test registers during normal operation, and SIRTEST must be cleared to 0 when loopback testing is finished. This feature reduces the amount of external coupling required during system test. If this bit is set to 1, and the SIRTEST bit is set to 0, the UARTTXD path is fed through to the UARTRXD path. In either SIR mode or UART mode, when this bit is set, the modem outputs are also fed through to the modem inputs. This bit is cleared to 0 on reset, to disable loopback. |
| 6:3 | - | Reserved, do not modify, read as zero. |
| 2 | SIRLP | SIR low-power IrDA mode. This bit selects the IrDA encoding mode. If this bit is cleared to 0, low-level bits are transmitted as an active high pulse with a width of 3/16th of the bit period. If this bit is set to 1, low-level bits are transmitted with a pulse width that is 3 times the period of the IrLPBaud16 input signal, regardless of the selected bit rate. Setting this bit uses less power, but might reduce transmission distances. |
| 1 | SIREN | SIR enable: 0 = IrDA SIR ENDEC is disabled. nSIROUT remains LOW (no light pulse generated), and signal transitions on SIRIN have no effect. 1 = IrDA SIR ENDEC is enabled. Data is transmitted and received on nSIROUT and SIRIN. UARTTXD remains HIGH, in the marking state. Signal transitions on UARTRXD or modem status inputs have no effect. This bit has no effect if the UARTEN bit disables the UART. |
| 0 | UARTEN | UART enable: 0 = UART is disabled. If the UART is disabled in the middle of transmission or reception, it completes the current character before stopping. 1 = the UART is enabled. Data transmission and reception occurs for either UART signals or SIR signals depending on the setting of the SIREN bit. |

**Note**

To enable transmission, the TXE bit and UARTEN bit must be set to 1. Similarly, to enable reception, the RXE bit and UARTEN bit, must be set to 1.

**Note**

Program the control registers as follows:

1. Disable the UART.
2. Wait for the end of transmission or reception of the current character.
3. Flush the transmit FIFO by setting the FEN bit to 0 in the Line Control Register, UARTLCR_H on page 3-12.
4. Reprogram the UARTCR Register.
5. Enable the UART.

### 3.3.9 Interrupt FIFO Level Select Register, UARTIFLS

The UARTIFLS Register is the interrupt FIFO level select register. You can use this register to define the FIFO level that triggers the assertion of UARTTXINTR and UARTRXINTR.

The interrupts are generated based on a transition through a level rather than being based on the level. That is, the interrupts are generated when the fill level progresses through the trigger level.

The bits are reset so that the trigger level is when the FIFOs are at the half-way mark. Table 3-13 lists the register bit assignments.

**Table 3-13 UARTIFLS Register**

| Bits | Name | Function |
|------|------|----------|
| 15:6 | - | Reserved, do not modify, read as zero. |
| 5:3 | RXIFLSEL | Receive interrupt FIFO level select. The trigger points for the receive interrupt are as follows: b000 = Receive FIFO becomes >= 1/8 full, b001 = Receive FIFO becomes >= 1/4 full, b010 = Receive FIFO becomes >= 1/2 full, b011 = Receive FIFO becomes >= 3/4 full, b100 = Receive FIFO becomes >= 7/8 full, b101-b111 = reserved. |
| 2:0 | TXIFLSEL | Transmit interrupt FIFO level select. The trigger points for the transmit interrupt are as follows: b000 = Transmit FIFO becomes <= 1/8 full, b001 = Transmit FIFO becomes <= 1/4 full, b010 = Transmit FIFO becomes <= 1/2 full, b011 = Transmit FIFO becomes <= 3/4 full, b100 = Transmit FIFO becomes <= 7/8 full, b101-b111 = reserved. |

### 3.3.10 Interrupt Mask Set/Clear Register, UARTIMSC

The UARTIMSC Register is the interrupt mask set/clear register. It is a read/write register.

On a read this register returns the current value of the mask on the relevant interrupt. On a write of 1 to the particular bit, it sets the corresponding mask of that interrupt. A write of 0 clears the corresponding mask.

All the bits are cleared to 0 when reset. Table 3-14 lists the register bit assignments.

**Table 3-14 UARTIMSC Register**

| Bits | Name | Function |
|------|------|----------|
| 15:11 | - | Reserved, read as zero, do not modify. |
| 10 | OEIM | Overrun error interrupt mask. A read returns the current mask for the UARTOEINTR interrupt. On a write of 1, the mask of the UARTOEINTR interrupt is set. A write of 0 clears the mask. |
| 9 | BEIM | Break error interrupt mask. A read returns the current mask for the UARTBEINTR interrupt. On a write of 1, the mask of the UARTBEINTR interrupt is set. A write of 0 clears the mask. |
| 8 | PEIM | Parity error interrupt mask. A read returns the current mask for the UARTPEINTR interrupt. On a write of 1, the mask of the UARTPEINTR interrupt is set. A write of 0 clears the mask. |
| 7 | FEIM | Framing error interrupt mask. A read returns the current mask for the UARTFEINTR interrupt. On a write of 1, the mask of the UARTFEINTR interrupt is set. A write of 0 clears the mask. |
| 6 | RTIM | Receive timeout interrupt mask. A read returns the current mask for the UARTRTINTR interrupt. On a write of 1, the mask of the UARTRTINTR interrupt is set. A write of 0 clears the mask. |
| 5 | TXIM | Transmit interrupt mask. A read returns the current mask for the UARTTXINTR interrupt. On a write of 1, the mask of the UARTTXINTR interrupt is set. A write of 0 clears the mask. |
| 4 | RXIM | Receive interrupt mask. A read returns the current mask for the UARTRXINTR interrupt. On a write of 1, the mask of the UARTRXINTR interrupt is set. A write of 0 clears the mask. |
| 3 | DSRMIM | nUARTDSR modem interrupt mask. A read returns the current mask for the UARTDSRINTR interrupt. On a write of 1, the mask of the UARTDSRINTR interrupt is set. A write of 0 clears the mask. |
| 2 | DCDMIM | nUARTDCD modem interrupt mask. A read returns the current mask for the UARTDCDINTR interrupt. On a write of 1, the mask of the UARTDCDINTR interrupt is set. A write of 0 clears the mask. |
| 1 | CTSMIM | nUARTCTS modem interrupt mask. A read returns the current mask for the UARTCTSINTR interrupt. On a write of 1, the mask of the UARTCTSINTR interrupt is set. A write of 0 clears the mask. |
| 0 | RIMIM | nUARTRI modem interrupt mask. A read returns the current mask for the UARTRIINTR interrupt. On a write of 1, the mask of the UARTRIINTR interrupt is set. A write of 0 clears the mask. |

### 3.3.11 Raw Interrupt Status Register, UARTRIS

The UARTRIS Register is the raw interrupt status register. It is a read-only register. This register returns the current raw status value, prior to masking, of the corresponding interrupt. A write has no effect.

**Caution**

All the bits, except for the modem status interrupt bits (bits 3 to 0), are cleared to 0 when reset. The modem status interrupt bits are undefined after reset.

Table 3-15 lists the register bit assignments.

**Table 3-15 UARTRIS Register**

| Bits | Name | Function |
|------|------|----------|
| 15:11 | - | Reserved, read as zero, do not modify. |
| 10 | OERIS | Overrun error interrupt status. Returns the raw interrupt state of the UARTOEINTR interrupt. |
| 9 | BERIS | Break error interrupt status. Returns the raw interrupt state of the UARTBEINTR interrupt. |
| 8 | PERIS | Parity error interrupt status. Returns the raw interrupt state of the UARTPEINTR interrupt. |
| 7 | FERIS | Framing error interrupt status. Returns the raw interrupt state of the UARTFEINTR interrupt. |
| 6 | RTRIS | Receive timeout interrupt status. Returns the raw interrupt state of the UARTRTINTR interrupt. (Note: In this case the raw interrupt cannot be set unless the mask is set, this is because the mask acts as an enable for power saving. That is, the same status can be read from UARTMIS and UARTRIS for the receive timeout interrupt.) |
| 5 | TXRIS | Transmit interrupt status. Returns the raw interrupt state of the UARTTXINTR interrupt. |
| 4 | RXRIS | Receive interrupt status. Returns the raw interrupt state of the UARTRXINTR interrupt. |
| 3 | DSRRMIS | nUARTDSR modem interrupt status. Returns the raw interrupt state of the UARTDSRINTR interrupt. |
| 2 | DCDRMIS | nUARTDCD modem interrupt status. Returns the raw interrupt state of the UARTDCDINTR interrupt. |
| 1 | CTSRMIS | nUARTCTS modem interrupt status. Returns the raw interrupt state of the UARTCTSINTR interrupt. |
| 0 | RIRMIS | nUARTRI modem interrupt status. Returns the raw interrupt state of the UARTRIINTR interrupt. |

### 3.3.12 Masked Interrupt Status Register, UARTMIS

The UARTMIS Register is the masked interrupt status register. It is a read-only register. This register returns the current masked status value of the corresponding interrupt. A write has no effect.

All the bits except for the modem status interrupt bits (bits 3 to 0) are cleared to 0 when reset. The modem status interrupt bits are undefined after reset. Table 3-16 lists the register bit assignments.

**Table 3-16 UARTMIS Register**

| Bits | Name | Function |
|------|------|----------|
| 15:11 | - | Reserved, read as zero, do not modify |
| 10 | OEMIS | Overrun error masked interrupt status. Returns the masked interrupt state of the UARTOEINTR interrupt. |
| 9 | BEMIS | Break error masked interrupt status. Returns the masked interrupt state of the UARTBEINTR interrupt. |
| 8 | PEMIS | Parity error masked interrupt status. Returns the masked interrupt state of the UARTPEINTR interrupt. |
| 7 | FEMIS | Framing error masked interrupt status. Returns the masked interrupt state of the UARTFEINTR interrupt. |
| 6 | RTMIS | Receive timeout masked interrupt status. Returns the masked interrupt state of the UARTRTINTR interrupt. |
| 5 | TXMIS | Transmit masked interrupt status. Returns the masked interrupt state of the UARTTXINTR interrupt. |
| 4 | RXMIS | Receive masked interrupt status. Returns the masked interrupt state of the UARTRXINTR interrupt. |
| 3 | DSRMMIS | nUARTDSR modem masked interrupt status. Returns the masked interrupt state of the UARTDSRINTR interrupt. |
| 2 | DCDMMIS | nUARTDCD modem masked interrupt status. Returns the masked interrupt state of the UARTDCDINTR interrupt. |
| 1 | CTSMMIS | nUARTCTS modem masked interrupt status. Returns the masked interrupt state of the UARTCTSINTR interrupt. |
| 0 | RIMMIS | nUARTRI modem masked interrupt status. Returns the masked interrupt state of the UARTRIINTR interrupt. |

### 3.3.13 Interrupt Clear Register, UARTICR

The UARTICR Register is the interrupt clear register and is write-only. On a write of 1, the corresponding interrupt is cleared. A write of 0 has no effect. Table 3-17 lists the register bit assignments.

**Table 3-17 UARTICR Register**

| Bits | Name | Function |
|------|------|----------|
| 15:11 | Reserved | Reserved, read as zero, do not modify. |
| 10 | OEIC | Overrun error interrupt clear. Clears the UARTOEINTR interrupt. |
| 9 | BEIC | Break error interrupt clear. Clears the UARTBEINTR interrupt. |
| 8 | PEIC | Parity error interrupt clear. Clears the UARTPEINTR interrupt. |
| 7 | FEIC | Framing error interrupt clear. Clears the UARTFEINTR interrupt. |
| 6 | RTIC | Receive timeout interrupt clear. Clears the UARTRTINTR interrupt. |
| 5 | TXIC | Transmit interrupt clear. Clears the UARTTXINTR interrupt. |
| 4 | RXIC | Receive interrupt clear. Clears the UARTRXINTR interrupt. |
| 3 | DSRMIC | nUARTDSR modem interrupt clear. Clears the UARTDSRINTR interrupt. |
| 2 | DCDMIC | nUARTDCD modem interrupt clear. Clears the UARTDCDINTR interrupt. |
| 1 | CTSMIC | nUARTCTS modem interrupt clear. Clears the UARTCTSINTR interrupt. |
| 0 | RIMIC | nUARTRI modem interrupt clear. Clears the UARTRIINTR interrupt. |

### 3.3.14 DMA Control Register, UARTDMACR

The UARTDMACR Register is the DMA control register. It is a read/write register. All the bits are cleared to 0 on reset. Table 3-18 lists the register bit assignments.

**Table 3-18 UARTDMACR Register**

| Bits | Name | Function |
|------|------|----------|
| 15:3 | - | Reserved, read as zero, do not modify. |
| 2 | DMAONERR | DMA on error. If this bit is set to 1, the DMA receive request outputs, UARTRXDMASREQ or UARTRXDMABREQ, are disabled when the UART error interrupt is asserted. |
| 1 | TXDMAE | Transmit DMA enable. If this bit is set to 1, DMA for the transmit FIFO is enabled. |
| 0 | RXDMAE | Receive DMA enable. If this bit is set to 1, DMA for the receive FIFO is enabled. |

### 3.3.15 Peripheral Identification Registers, UARTPeriphID0-3

The UARTPeriphID0-3 Registers are four 8-bit registers, that span address locations 0xFE0 - 0xFEC. The registers can conceptually be treated as a 32-bit register. The read only registers provide the following options of the peripheral:

**PartNumber[11:0]** Identifies the peripheral. This is 0x011 for the UART.

**Designer ID[19:12]** Identifies the designer. This is set to 0x41, to indicate that ARM designed the peripheral.

**Revision[23:20]** The peripheral revision number is revision-dependent. See Table 3-21 on page 3-24.

**Configuration[31:24]** The configuration option of the peripheral. The configuration value is 0.

[FIGURE 3-1: Peripheral Identification Register bit assignments]

Components:
- Actual register bit assignment showing Configuration (7-0), Revision number (7-4), Designer 1 (3-0), Designer 0 (7-4), Part number 1 (3-0), Part number 0 (7-0)
- Conceptual register bit assignment showing bits 31-0 with Configuration, Revision number, Designer, and Part number fields

**Note**

When you design a systems memory map you must remember that the register has a 4KB memory footprint. All memory accesses to the Peripheral Identification Registers must be 32-bit, using the LDR and STR instructions.

The four, 8-bit Peripheral Identification Registers are described in the following subsections:

- UARTPeriphID0 Register
- UARTPeriphID1 Register on page 3-24
- UARTPeriphID2 Register on page 3-24
- UARTPeriphID3 Register on page 3-25.

#### UARTPeriphID0 Register

The UARTPeriphID0 Register is hard coded and the fields in the register determine the reset value. Table 3-19 lists the register bit assignments.

**Table 3-19 UARTPeriphID0 Register**

| Bits | Name | Description |
|------|------|-------------|
| 15:8 | - | Reserved, read undefined must read as zeros |
| 7:0 | PartNumber0 | These bits read back as 0x11 |

#### UARTPeriphID1 Register

The UARTPeriphID1 Register is hard coded and the fields in the register determine the reset value. Table 3-20 lists the register bit assignments.

**Table 3-20 UARTPeriphID1 Register**

| Bits | Name | Description |
|------|------|-------------|
| 15:8 | - | Reserved, read undefined, must read as zeros |
| 7:4 | Designer0 | These bits read back as 0x1 |
| 3:0 | PartNumber1 | These bits read back as 0x0 |

#### UARTPeriphID2 Register

The UARTPeriphID2 Register is hard coded and the fields in the register determine the reset value. Table 3-21 lists the register bit assignments.

**Table 3-21 UARTPeriphID2 Register**

| Bits | Name | Description |
|------|------|-------------|
| 15:8 | - | Reserved, read undefined, must read as zeros |
| 7:4 | Revision | This field depends on the revision of the UART: r1p0 = 0x0, r1p1 = 0x1, r1p3 = 0x2, r1p4 = 0x2, r1p5 = 0x3 |
| 3:0 | Designer1 | These bits read back as 0x4 |

#### UARTPeriphID3 Register

The UARTPeriphID3 Register is hard coded and the fields in the register determine the reset value. Table 3-22 lists the register bit assignments.

**Table 3-22 UARTPeriphID3 Register**

| Bits | Name | Description |
|------|------|-------------|
| 15:8 | - | Reserved, read undefined, must read as zeros |
| 7:0 | Configuration | These bits read back as 0x00 |

### 3.3.16 PrimeCell Identification Registers, UARTPCellID0-3

The UARTPCellID0-3 Registers are four 8-bit wide registers, that span address locations 0xFF0-0xFFC. The registers can conceptually be treated as a 32-bit register. The register is used as a standard cross-peripheral identification system. The UARTPCellID Register is set to 0xB105F00D.

[FIGURE 3-2: PrimeCell Identification Register bit assignments]

Components:
- Actual register bit assignment showing UARTPCellID3 (7-0), UARTPCellID2 (7-0), UARTPCellID1 (7-0), UARTPCellID0 (7-0)
- Conceptual register bit assignment showing bits 31-0 with UARTPCellID3, UARTPCellID2, UARTPCellID1, UARTPCellID0 fields

The four, 8-bit PrimeCell Identification Registers are described in the following subsections:

- UARTPCellID0 Register on page 3-26
- UARTPCellID1 Register on page 3-26
- UARTPCellID2 Register on page 3-26
- UARTPCellID3 Register on page 3-27.

#### UARTPCellID0 Register

The UARTPCellID0 Register is hard coded and the fields in the register determine the reset value. Table 3-23 lists the register bit assignments.

**Table 3-23 UARTPCellID0 Register**

| Bits | Name | Description |
|------|------|-------------|
| 15:8 | - | Reserved, read undefined, must read as zeros |
| 7:0 | UARTPCellID0 | These bits read back as 0x0D |

#### UARTPCellID1 Register

The UARTPCellID1 Register is hard coded and the fields in the register determine the reset value. Table 3-24 lists the register bit assignments.

**Table 3-24 UARTPCellID1 Register**

| Bits | Name | Description |
|------|------|-------------|
| 15:8 | - | Reserved, read undefined, must read as zeros |
| 7:0 | UARTPCellID1 | These bits read back as 0xF0 |

#### UARTPCellID2 Register

The UARTPCellID2 Register is hard coded and the fields in the register determine the reset value. Table 3-25 lists the register bit assignments.

**Table 3-25 UARTPCellID2 Register**

| Bits | Name | Description |
|------|------|-------------|
| 15:8 | - | Reserved, read undefined, must read as zeros |
| 7:0 | UARTPCellID2 | These bits read back as 0x05 |

#### UARTPCellID3 Register

The UARTPCellID3 Register is hard coded and the fields in the register determine the reset value. Table 3-26 lists the register bit assignments.

**Table 3-26 UARTPCellID3 Register**

| Bits | Name | Description |
|------|------|-------------|
| 15:8 | - | Reserved, read undefined, must read as zeros |
| 7:0 | UARTPCellID3 | These bits read back as 0xB1 |

---

