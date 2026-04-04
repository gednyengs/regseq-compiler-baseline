# PL011 Driver Benchmarking Strategy: Stock vs. Custom

## Overview

This document outlines a strategy for comparing the Linux kernel's production PL011 UART driver (`drivers/tty/serial/amba-pl011.c`) against a custom implementation, running on an emulated ARM64 (QEMU `virt` machine). The focus is on interrupt handling efficiency, FIFO utilization, and CPU cost — the dimensions where UART driver differences are most meaningful.

## Baseline Driver: The Kernel's amba-pl011.c

The baseline for comparison is the kernel's own `drivers/tty/serial/amba-pl011.c` — a manually written, production-grade driver that runs on every ARM system with a PL011 (Raspberry Pi, ARM server platforms, etc.). This is a well-optimized, battle-tested driver with significant sophistication:

- DMA support with automatic fallback to PIO
- Configurable FIFO trigger thresholds via vendor data
- Careful interrupt coalescing
- Console attachment and power management

This sets a high bar. If the custom driver outperforms it on any metric, that's a meaningful result.

The stock driver should be built into the kernel (`=y`, not `=m`) so it handles the console UART from boot. Ensure DMA-related options are also enabled to avoid benchmarking it in a degraded mode:

- `CONFIG_SERIAL_AMBA_PL011=y`
- `CONFIG_DMA_ENGINE=y`
- `CONFIG_AMBA_PL011_DMA=y` (if available as a separate option in your kernel version)

## Phase 0 — Building the Kernel and Rootfs

### Kernel Source

Clone the stable kernel from kernel.org:

```bash
git clone --depth 1 --branch v6.12 \
  https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git
cd linux
```

Use `--depth 1` to avoid downloading the full history. Check [kernel.org](https://kernel.org) for the latest stable tag.

### Cross-Compiler

Install the ARM64 cross-compilation toolchain. On Ubuntu/Debian:

```bash
sudo apt install gcc-aarch64-linux-gnu
```

### Kernel Configuration

Start with the default ARM64 config, which already includes PL011 support, virtio, and what's needed for QEMU's `virt` machine:

```bash
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- defconfig
```

Then customize for benchmarking:

```bash
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- menuconfig
```

Key options to set:

- `CONFIG_SERIAL_AMBA_PL011=y` — built-in so the console UART works from boot (see "Driver Binding" section below for how the custom driver claims the test UART)
- `CONFIG_DMA_ENGINE=y` — required for the stock driver's DMA path
- `CONFIG_AMBA_PL011_DMA=y` — if available as a separate option
- `CONFIG_VIRTIO_BLK=y` — for the rootfs disk
- `CONFIG_NET_9P=y` and `CONFIG_9P_FS=y` — optional but handy for sharing files between host and guest without rebuilding the rootfs

### Build

```bash
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- -j$(nproc) Image modules
```

The kernel image lands at `arch/arm64/boot/Image` — this is what you pass to QEMU's `-kernel` flag.

### Building the Custom Driver

Build the custom PL011 driver as an out-of-tree module against the same kernel tree:

```bash
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- \
  -C /path/to/linux M=/path/to/custom_driver modules
```

This produces a `.ko` file you can `insmod` inside the guest.

### Rootfs

Buildroot is the best fit for this use case, since you can bake all test tools directly into the image:

```bash
git clone https://github.com/buildroot/buildroot.git
cd buildroot
make qemu_aarch64_virt_defconfig
make menuconfig
```

In Buildroot's menuconfig, enable the packages you need for benchmarking:

- `BR2_PACKAGE_PIPEVIEWER=y` (pv)
- `BR2_PACKAGE_STRESS_NG=y`
- `BR2_PACKAGE_PERF=y` (under kernel tools)
- `BR2_PACKAGE_IPERF3=y` (if also doing network testing)

Then build:

```bash
make -j$(nproc)
```

The rootfs image will be at `output/images/rootfs.ext4`. Convert to raw if needed:

```bash
qemu-img convert -f raw output/images/rootfs.ext4 rootfs.img
```

## Driver Binding: How the Custom Driver Claims the Test UART

### The Problem

The QEMU `virt` machine creates two PL011 UARTs, both with the same AMBA peripheral ID (`0x00041011`). The stock built-in driver matches this ID and will claim both devices at boot. The custom module, loaded later, would find no unclaimed device to bind to.

### The Solution: Device Tree Partitioning

Use a modified device tree to give the test UART a different `compatible` string that only the custom driver recognizes. The stock driver ignores the test UART at boot, leaving it unclaimed until the custom module loads.

**Step 1 — Extract and modify QEMU's device tree:**

```bash
# Dump the default DTB
qemu-system-aarch64 -machine virt,dumpdtb=virt.dtb -cpu cortex-a53
dtc -I dtb -O dts virt.dtb > virt.dts
```

In `virt.dts`, find the second UART node (the test UART). It will look something like:

```dts
pl011@9040000 {
    compatible = "arm,pl011", "arm,primecell";
    reg = <0x00 0x9040000 0x00 0x1000>;
    interrupts = <0x00 0x08 0x04>;
    clock-names = "uartclk", "apb_pclk";
    ...
};
```

Change its `compatible` to a custom string:

```dts
pl011@9040000 {
    compatible = "custom,pl011", "arm,primecell";
    ...
};
```

Leave the first UART (console) unchanged at `"arm,pl011"`.

Recompile:

```bash
dtc -I dts -O dtb virt.dts > virt-custom.dtb
```

**Step 2 — Match the custom string in your driver:**

In the custom driver source, add an OF match table for the custom compatible string, alongside the standard AMBA ID match:

```c
static const struct of_device_id custom_pl011_of_match[] = {
    { .compatible = "custom,pl011" },
    { },
};
MODULE_DEVICE_TABLE(of, custom_pl011_of_match);

static struct amba_id custom_pl011_ids[] = {
    { .id = 0x00041011, .mask = 0x000fffff },
    { 0, 0 },
};
```

### The Boot Sequence

1. Kernel boots with the stock PL011 driver built-in.
2. Stock driver sees the first UART (`compatible = "arm,pl011"`), claims it → `ttyAMA0` (console) works immediately.
3. Stock driver sees the second UART (`compatible = "custom,pl011"`), does not recognize it, skips it.
4. The second UART sits unclaimed.
5. User loads the custom module: `insmod custom_pl011.ko`
6. Custom driver matches on `"custom,pl011"`, the AMBA bus probes it, and the custom driver takes ownership of the test UART cleanly.

This ensures the console is always stable on the stock driver, and the test UART is exclusively owned by whichever custom module you load.

### Alternative: driver_override via sysfs

If modifying the device tree is inconvenient, you can unbind and rebind at runtime:

```bash
echo "custom_pl011" > /sys/bus/amba/devices/<device-id>/driver_override
echo "<device-id>" > /sys/bus/amba/drivers/amba-pl011/unbind
echo "<device-id>" > /sys/bus/amba/drivers/custom_pl011/bind
```

This works but is messier — there is a brief window where the test UART has no driver, and the stock driver will have already initialized the hardware in its own way. The device tree approach is preferred because the stock driver never touches the test UART in the first place.

## Phase 1 — Environment Setup

Use the QEMU `virt` machine with two PL011 UARTs: one for console, one dedicated to testing. This keeps measurement traffic isolated from kernel log noise.

```bash
qemu-system-aarch64 \
  -machine virt \
  -cpu cortex-a53 \
  -m 1024 \
  -kernel Image \
  -dtb virt-custom.dtb \
  -append "root=/dev/vda console=ttyAMA0" \
  -drive file=rootfs.img,format=raw,if=virtio \
  -serial mon:stdio \
  -serial tcp::4321,server,nowait
```

`ttyAMA0` is the console, owned by the stock built-in driver. `ttyAMA1` is the test port, left unclaimed at boot and accessible from the host on TCP 4321.

Load the custom driver to claim the test UART:

```bash
insmod /path/to/custom_pl011.ko
```

Pin QEMU's vCPU threads to dedicated host cores and set the host CPU governor to `performance` to reduce scheduling jitter.

## Phase 2 — What to Measure

There are five dimensions that matter for a UART driver comparison.

### 2.1 Throughput at Configured Baud Rate

The theoretical max is baud/10 bytes per second (8 data bits + start + stop). Measure how close each driver actually gets, and whether it sustains that rate over long transfers. Use `pv` (pipe viewer) for measurement:

```bash
# On the guest, send data out ttyAMA1
dd if=/dev/urandom bs=4096 count=1000 | pv > /dev/ttyAMA1

# On the host, receive and measure
nc localhost 4321 | pv > /dev/null
```

Run at multiple baud rates (115200, 921600, 1500000, 3000000) to see where each driver starts falling behind the theoretical line.

### 2.2 Byte-Level Latency

Measures how quickly a single byte written to the port arrives at the other end. Timestamp on both sides with microsecond precision:

```c
// Simple C program on the guest
clock_gettime(CLOCK_MONOTONIC, &before);
write(fd, &byte, 1);
tcdrain(fd);  // wait for hardware to finish
clock_gettime(CLOCK_MONOTONIC, &after);
```

On the host side, use a matching program that timestamps arrival. Run thousands of iterations and build a histogram — the distribution matters more than the mean, since UART latency often has a bimodal shape depending on whether the byte hits a full or empty FIFO.

### 2.3 Interrupt Rate and Cost

This is usually where the real driver differences show up. A naive PL011 driver fires an interrupt per byte; a good one uses the FIFO trigger levels to batch interrupts.

Monitor during a sustained transfer:

```bash
# Snapshot interrupts before and after a timed transfer
cat /proc/interrupts > before.txt
# ... run transfer for 30 seconds ...
cat /proc/interrupts > after.txt
```

Compute interrupts-per-byte for each driver. Also measure CPU cost per interrupt using `perf`:

```bash
perf stat -e cycles,instructions -a -- sleep 30
# while running transfer in parallel
```

Capture flamegraphs during the transfer to see where each driver spends time:

```bash
perf record -g -a -- sleep 10
perf script | stackcollapse-perf.pl | flamegraph.pl > pl011_${driver}.svg
```

### 2.4 FIFO Behavior

The PL011 has a 16-entry (or 32-entry in some revisions) RX/TX FIFO with programmable trigger levels. Measure how each driver configures these. Overrun events tell you if the driver isn't draining the RX FIFO fast enough:

```bash
cat /proc/tty/driver/ttyAMA
```

This gives `tx`, `rx`, `oe` (overrun errors), `fe` (framing errors), and other counters. Flood the port at max rate and see which driver starts dropping data first.

### 2.5 Behavior Under CPU Contention

Run a CPU-intensive background workload while transferring data:

```bash
stress-ng --cpu 4
```

A well-written driver with proper FIFO and interrupt coalescing will degrade gracefully; a byte-at-a-time driver will start losing data because interrupts get serviced too slowly.

## Phase 3 — Test Matrix

Run every combination systematically:

| Test                      | Baud Rates         | Payload Sizes | With CPU Load |
| ------------------------- | ------------------ | ------------- | ------------- |
| Sustained TX throughput   | 115200, 921600, 3M | 64B, 4KB, 1MB | No, Yes       |
| Sustained RX throughput   | 115200, 921600, 3M | 64B, 4KB, 1MB | No, Yes       |
| Byte latency (idle)       | 115200, 921600     | 1 byte        | No            |
| Byte latency (under load) | 115200, 921600     | 1 byte        | Yes           |
| Interrupt count per MB    | 115200, 921600, 3M | 1MB           | No            |
| Overrun threshold         | 921600, 3M         | flood         | No, Yes       |

10 runs per cell. Report medians and p95.

## Phase 4 — Automation and Analysis

### Automation Script

Since the stock driver is built-in and owns the console UART (`ttyAMA0`), while the custom driver is a loadable module that owns the test UART (`ttyAMA1`), the automation runs benchmarks on `ttyAMA0` for the stock driver and `ttyAMA1` for the custom driver. Both UARTs are the same emulated PL011 hardware, so the comparison is valid.

```bash
mkdir -p results

# Benchmark the stock built-in driver on ttyAMA0
# (Console output should be redirected or suppressed during the test)
STOCK_DEV=/dev/ttyAMA0
for baud in 115200 921600 3000000; do
    stty -F $STOCK_DEV $baud raw
    for run in $(seq 1 10); do
        cat /proc/interrupts > results/stock_${baud}_${run}_before.txt
        dd if=/dev/urandom bs=4096 count=256 > $STOCK_DEV 2> results/stock_${baud}_${run}_throughput.txt
        cat /proc/interrupts > results/stock_${baud}_${run}_after.txt
        cat /proc/tty/driver/ttyAMA > results/stock_${baud}_${run}_tty.txt
    done
done

# Load the custom driver and benchmark on ttyAMA1
insmod /path/to/custom_pl011.ko
sleep 2
CUSTOM_DEV=/dev/ttyAMA1
for baud in 115200 921600 3000000; do
    stty -F $CUSTOM_DEV $baud raw
    for run in $(seq 1 10); do
        cat /proc/interrupts > results/custom_${baud}_${run}_before.txt
        dd if=/dev/urandom bs=4096 count=256 > $CUSTOM_DEV 2> results/custom_${baud}_${run}_throughput.txt
        cat /proc/interrupts > results/custom_${baud}_${run}_after.txt
        cat /proc/tty/driver/ttyAMA > results/custom_${baud}_${run}_tty.txt
    done
done
```

Note: when benchmarking the stock driver on `ttyAMA0`, kernel log messages may interfere with measurements. Consider suppressing them with `dmesg -n 1` before running the stock benchmarks, then restoring with `dmesg -n 7` afterward.

### Key Comparisons

- **Throughput as % of theoretical max** — raw efficiency
- **Interrupts per byte** — FIFO/batching strategy
- **CPU cycles per byte** (from `perf`) — code path efficiency
- **Overrun threshold under CPU contention** — robustness

## Phase 5 — QEMU Caveats

QEMU's PL011 emulation (`hw/char/pl011.c`) does not model real timing. Writes to the TX FIFO complete instantly rather than at baud-rate speed, and the FIFO fill/drain timing is approximate.

This means absolute throughput numbers aren't meaningful in hardware terms — but the **relative comparison** between two drivers is still valid, since both are running against the same emulation.

If realistic timing is needed, two options are available:

1. Patch QEMU's PL011 model to throttle TX to actual baud rate
2. Move to real hardware (e.g., Raspberry Pi 4, which has a PL011)