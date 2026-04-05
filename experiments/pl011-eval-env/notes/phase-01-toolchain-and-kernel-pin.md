# Phase 1 Result And Phase 2 Source Pin

## Status

Phase 1 is complete.

The host-side `aarch64` cross-compilation toolchain is installed and verified.

Verified binaries:

- `aarch64-linux-gnu-gcc`
- `aarch64-linux-gnu-g++`
- `aarch64-linux-gnu-ld`

Verified packages on the Ubuntu host:

- `binutils-aarch64-linux-gnu`
- `gcc-aarch64-linux-gnu`
- `g++-aarch64-linux-gnu`

Verified compiler version:

- GCC/G++ `13.3.0`

## Commands Used For Verification

```bash
which aarch64-linux-gnu-gcc aarch64-linux-gnu-g++ aarch64-linux-gnu-ld
dpkg -l | rg 'gcc-aarch64-linux-gnu|g\+\+-aarch64-linux-gnu|binutils-aarch64-linux-gnu'
aarch64-linux-gnu-gcc --version
aarch64-linux-gnu-g++ --version
```

## Phase 2 Source Selection

The kernel source line is pinned to:

- `v6.18.21`

Reason:

- the `6.18` line aligns with the version family used in the driver-analysis
  corpus work
- `v6.18.21` gives an exact reproducible revision rather than an ambiguous
  series-level target

## Planned Clone Command

```bash
mkdir -p ~/regseq-eval/pl011/kernel-src
cd ~/regseq-eval/pl011/kernel-src

git clone --depth 1 --branch v6.18.21 \
  https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git \
  pl011-arm64-linux
```

## Planned Verification After Clone

```bash
cd ~/regseq-eval/pl011/kernel-src/pl011-arm64-linux
git describe --tags --always
git rev-parse HEAD
```

## Next Question

After the source tree is cloned, the next environment decision is the kernel
configuration baseline for the PL011 QEMU `virt` bring-up path.
