# 05 — Linux Native-Pilot Runbook (x86_64 + KVM)

**Role**: the concrete, reproducible procedure to build the Linux E1000
evaluation environment and collect the **native (stock `e1000`) baseline** for
the headline workloads, and to fix the preregistered guardrail band. This is
the operational realization of `02-minimal-sufficient-evaluation.md` §7 and
must not contradict it; it obeys the hard invariant of
`03-cut-list-and-technical-justification.md` §3 (no QEMU device-model patch).

**Scope**: the **native-only pilot** — stock driver only. It (i) stands up the
environment, (ii) proves the harness via a smoke gate, (iii) collects native
baselines for W1–W3 (headline) plus W4/W5 (Linux auxiliary), (iv) computes and
**preregisters** the W2/W3 guardrail band. It is **not** the Regseq-vs-native
comparison: that is a later campaign that reuses this exact harness unchanged.

## 1. Host assumptions

- Ubuntu 24.04, `x86_64`, **KVM available** (`/dev/kvm` present), bridge/TAP
  permitted. macOS is not a measurement host.
- Working tree under the project convention: `~/regseq-eval/e1000/` with
  subdirs `{qemu-src,kernel-src,rootfs-src,images,generated,results,scripts}`.

## 2. Environment build (Loop A — host build)

All artifacts are **pinned and SHA-256-recorded**. The pilot and the future
comparison campaign must use byte-identical QEMU/kernel/rootfs.

### 2.1 QEMU (stock v8.2.2, x86_64 target, no device-model patch)

Build `qemu-system-x86_64` from the already-pinned, upstream-clean v8.2.2 tree
(`~/regseq-eval/pl011/qemu-src/qemu`, git tag `v8.2.2`, `hw/net/e1000.c`
unmodified). Do **not** apply any e1000/device-model patch (hard invariant).

```
cd ~/regseq-eval/pl011/qemu-src/qemu
git status --porcelain hw/net/e1000.c hw/net/e1000x_common.c   # must be empty
./configure --target-list=x86_64-softmmu --enable-kvm
make -j"$(nproc)"
install -D build/qemu-system-x86_64 ~/regseq-eval/e1000/qemu-src/qemu-system-x86_64
sha256sum ~/regseq-eval/e1000/qemu-src/qemu-system-x86_64   # record
```

Record: QEMU version (`--version`), the v8.2.2 commit, the binary SHA-256.

### 2.2 Kernel (project-controlled x86_64, stock `e1000` as a module)

Use the same kernel version as the PL011 campaign (Linux v6.18.21) for
cross-device consistency, built `x86_64` into a project-controlled tree at
`~/regseq-eval/e1000/kernel-src/e1000-x86_64-linux`. Required config (the
critical point is **`CONFIG_E1000=m`** so the stock driver can be
blacklisted/`driver_override`d, never built-in):

```
CONFIG_E1000=m            # stock e1000 as a loadable module (NOT =y)
CONFIG_PCI=y
CONFIG_VIRTIO_PCI=y CONFIG_VIRTIO_BLK=y CONFIG_VIRTIO_NET=y   # boot disk + mgmt NIC
CONFIG_KVM_GUEST=y
CONFIG_SERIAL_8250=y CONFIG_SERIAL_8250_CONSOLE=y             # ttyS0 console
CONFIG_NET_9P=y CONFIG_9P_FS=y                                 # artifact transfer (optional)
```

Record: kernel version, config SHA-256, build commit.

### 2.3 Rootfs (Buildroot 2026.02, x86_64, test tools baked in)

`qemu_x86_64_defconfig` + a fragment enabling: `iperf3`, `netperf` (or
`sockperf`), `tcpdump`, `ethtool`, `iproute2`, a DHCP client (`udhcpc` via
busybox or `dhcpcd`), `perf`, `stress-ng`. Output
`~/regseq-eval/e1000/images/rootfs.ext4`; record image SHA-256.

## 3. QEMU topology and fairness (binding contract)

One DUT e1000, one virtio-net management NIC, a netns peer, console+QMP
control. QEMU's default `-device e1000` is the 82540EM (PCI `0x8086:0x100E`,
verified in the v8.2.2 tree). Every command below is concrete and pinned; the
host core map is **enforced**, not merely recorded.

### 3.1 Host peer (network namespace, reproducible and offload-clean)

```
HOSTCORES_QEMU=2-3        # enforced: QEMU vCPUs pinned here
HOSTCORE_PEER=5           # enforced: peer iperf/netserver pinned here
QMP=$HOME/regseq-eval/e1000/results/qmp.sock     # no '~' after unix:

sudo ip netns add e1000peer
sudo ip link add br-e1000 type bridge
sudo ip tuntap add tap-dut mode tap user "$USER"
sudo ip link add veth-h type veth peer name veth-p
sudo ip link set veth-p netns e1000peer
sudo ip link set tap-dut master br-e1000
sudo ip link set veth-h  master br-e1000
sudo ip link set tap-dut up; sudo ip link set veth-h up
sudo ip link set br-e1000 up
sudo ip -n e1000peer addr add 192.168.77.1/24 dev veth-p
sudo ip -n e1000peer link set veth-p up; sudo ip -n e1000peer link set lo up
# offloads OFF on every host-side endpoint (tap, bridge, veth pair):
for i in tap-dut br-e1000 veth-h; do sudo ethtool -K $i tso off gso off gro off lro off rx off tx off; done
sudo ip netns exec e1000peer ethtool -K veth-p tso off gso off gro off lro off rx off tx off
# DHCP for W1, iperf3 server for W2/W3, capture for W5 — all in the netns,
# the peer pinned to its core:
sudo ip netns exec e1000peer dnsmasq --interface=veth-p --bind-interfaces \
  --dhcp-range=192.168.77.50,192.168.77.100,1h --no-daemon &
# persistent servers (NOT one-shot: reused across >=10 W2 runs and W4 reuse),
# each pinned to the peer core:
sudo ip netns exec e1000peer taskset -c $HOSTCORE_PEER iperf3 -s &        # W2
sudo ip netns exec e1000peer taskset -c $HOSTCORE_PEER netserver -D &     # W3 (netperf)
# (or sockperf -i ... server for the W3 ping-pong variant)
# teardown: pkill dnsmasq/iperf3 in netns; ip netns del e1000peer;
#           ip link del br-e1000; ip link del veth-h; ip tuntap del tap-dut mode tap
```

Archive `ethtool -k` for tap-dut, br-e1000, veth-h, veth-p **and** the guest
DUT (§5) in every run manifest; a non-`off` offload on any of them voids the
run.

### 3.2 The concrete QEMU command line (verbatim, recorded per run)

```
KSRC=$HOME/regseq-eval/e1000/kernel-src/e1000-x86_64-linux
taskset -c $HOSTCORES_QEMU \
$HOME/regseq-eval/e1000/qemu-src/qemu-system-x86_64 \
  -machine q35 -enable-kvm -cpu host -m 2048 -smp 2 \
  -kernel $KSRC/arch/x86/boot/bzImage \
  -append "root=/dev/vda rw console=ttyS0 modprobe.blacklist=e1000 nokaslr" \
  -drive file=$HOME/regseq-eval/e1000/images/rootfs.ext4,if=virtio,format=raw \
  -netdev tap,id=dutnet,ifname=tap-dut,script=no,downscript=no \
  -device e1000,netdev=dutnet,id=dutnic,addr=0x4 \
  -netdev user,id=mgmt \
  -device virtio-net-pci,netdev=mgmt,id=mgmtnic,addr=0x5 \
  -qmp unix:$QMP,server,nowait \
  -display none -serial stdio
```

- `addr=0x4` pins the DUT PCI slot → deterministic guest BDF (record it).
- Management is `-netdev user` on a **separate** virtio-net device; management
  traffic is never measured and never on the DUT path (per `02` §7). It keeps
  the guest usable while the DUT e1000 is unbound.
- `modprobe.blacklist=e1000` in `-append` is what makes the binding-fairness
  contract hold (stock e1000 never auto-probes the DUT at boot).
- `console=ttyS0` + `-serial stdio` is the x86_64 console; `-qmp unix:$QMP`
  (no `~`) is the control plane for W4 `set_link`. QMP/monitor are host
  control plane, **not** a device-model patch (`03` §3) — permitted.
- `set_link`'s target string is the **NIC net-client name**, which equals the
  e1000 device id: use **`dutnic`** consistently (verified against the v8.2.2
  QAPI/`net.c` name lookup).

### 3.3 Binding fairness (stock driver bound explicitly, proven clean)

```
BDF=$(...)                                   # derive from addr=0x4; record it
cp /var/log/dmesg boot-dmesg.txt 2>/dev/null || dmesg > boot-dmesg.txt
test -e /sys/bus/pci/devices/$BDF/driver && echo "FAIL: DUT pre-bound" && exit 1
echo e1000 > /sys/bus/pci/devices/$BDF/driver_override
# `modprobe.blacklist=e1000` (the boot append) stops AUTO-load so the DUT is
# never probed at boot, but it also blocks modprobe/drivers_probe from loading
# the module on demand. Bind deterministically by loading the module object
# directly (insmod bypasses the autoload blacklist; there is exactly one
# e1000 — mgmt is virtio-net — so it binds only the DUT):
insmod /lib/modules/$(uname -r)/kernel/drivers/net/ethernet/intel/e1000/e1000.ko
dmesg > postbind-dmesg.txt                   # archive; the boot→postbind delta
                                             # is the "no prior probe" proof
lspci -nnk -s $BDF                           # must show e1000 bound, 8086:100e
```

The boot-dmesg / postbind-dmesg **delta** (not a `grep`) is the archived proof
that no driver touched the DUT before the explicit override. Identical QEMU
command line is used for native and the future Regseq run; only the bound
module differs; the cmdline is recorded verbatim per run.

## 4. Smoke gate (Loop B — prove the harness before collecting data)

Pass/fail gate; no measurement. Boot the VM; confirm the DUT e1000 is
**unbound at boot** (blacklist works); bind stock `e1000` via the §3 sequence;
`ip link set <dut> up`; acquire an address (DHCP from the netns dnsmasq, or
static); `ping` the peer. Success = bound + link up + ICMP echo. Capture
`lspci -nnk`, `dmesg`, `ip -s link`. If this fails, **do not proceed** to
Loop C; fix the harness first.

## 5. Native baseline collection (Loop C — stock driver only)

For every workload: stock `e1000` bound per §3; record the full per-run
manifest (§7); statistical contract per §6. Workloads are exactly W1–W5 of
`02` §3; only the headline rows W1–W3 set the guardrail.

- **W1 — driver cold-start** (pass/fail, ≥5 runs): the cold-start under test
  is the *driver's* probe→open→address path, not Linux boot. Each run is a
  module cold-load on a clean device — `rmmod e1000` → `driver_override` →
  `insmod e1000.ko` (clean probe) → `ip link up` → DHCP/static →
  `ping <peer>`. A full guest reboot would mostly re-test the OS boot, not
  the driver, and is deliberately not used; the module cold-load is the
  faithful, isolating cold-start for a driver-DSL claim. Record: bind, address
  acquired, ICMP loss.
- **W2 — bulk TCP @ 1500 MTU, TX-csum on** (metric, ≥10 runs): set MTU 1500;
  pin the offload surface to exactly the intended workload —
  `ethtool -K <dut> tx on tso off gso off gro off lro off` — and archive
  `ethtool -k <dut>` (TSO/GSO/GRO/LRO are out of scope by the variant pin;
  leaving them on would silently measure large-segment paths and **void** the
  guardrail). Run bidirectional `iperf3` (guest⇄peer) for a fixed duration for
  the throughput/CPU numbers; **separately**, transfer a fixed known file and
  compare `sha256sum` at both ends for the payload-integrity verdict (iperf3
  alone does not give an end-to-end payload hash). Record: throughput
  (Gbit/s); guest CPU cost per byte from `perf stat -a -e cycles,instructions`
  (or `/proc/stat` delta) over the run; integrity pass/fail.
- **W3 — request/response + RTT** (metric, ≥10 runs): `netperf -t TCP_RR`
  (or `sockperf` ping-pong) + a `ping` RTT sample. Record: p50/p95/p99 RTT and
  transaction rate.
- **W4 — link flap** (pass/fail, ≥5 runs, Linux): start `iperf3`; open the QMP
  socket and complete the handshake **before** any command —
  `{"execute":"qmp_capabilities"}` — then
  `{"execute":"set_link","arguments":{"name":"dutnic","up":false}}`; wait;
  `{"execute":"set_link","arguments":{"name":"dutnic","up":true}}`; re-acquire
  address; resume. Note `name` is the NIC net-client name (`dutnic`), used
  consistently. Record: no corruption, no driver hang, traffic resumes.
  (`set_link` is patch-free; stock QEMU raises the E1000 LSC on link-status
  change — `03` §2.5.)
- **W5 — filtering/promiscuous** (pass/fail, Linux only): peer injects
  unicast / multicast / non-matching frames; capture with `tcpdump` on the DUT
  with promiscuous off then on (`ip link set <dut> promisc on/off`). Record:
  filtered set vs. promiscuous set.

## 6. Statistical contract (scoped to this pilot)

Per `02` §6: pass/fail workloads (W1, W4, W5) ≥5 independent runs,
non-determinism reported not averaged; metric workloads (W2, W3) ≥10 runs,
report median + p95/p99 with a bootstrap 95% CI. Interleaving / sentinels /
drift gates are deferred to the comparison campaign, **but the full per-run
manifest (§7) is captured from run 1** so this pilot can be retro-validated
and the drift template adopted later without re-baselining.

## 7. Per-run metadata manifest (captured from run 1)

One JSON per run, archived under
`~/regseq-eval/e1000/results/campaigns/2026-05-19-native-pilot/`:

- host: kernel, CPU model, governor, SMT/turbo, `/dev/kvm` present, NUMA/IRQ
  pinning, and the **enforced** core map (`HOSTCORES_QEMU` for QEMU vCPUs,
  `HOSTCORE_PEER` for the peer iperf/netserver) — the values actually
  `taskset`-applied, not just intended;
- qemu: binary SHA-256, `--version`, the v8.2.2 commit, the **verbatim QEMU
  command line**;
- guest: kernel version + config SHA-256, rootfs SHA-256, `e1000` module
  version, `lspci -nnk -s <BDF>` output, the DUT BDF, and the
  **boot-dmesg / postbind-dmesg delta** (the "no prior probe" proof — full
  files, not a grep);
- network: bridge/tap/veth/netns names and layout, peer address, MTU, and
  `ethtool -k` snapshots for tap-dut, br-e1000, veth-h, veth-p (every offload
  must read `off`) **and** the guest DUT, which must match the **per-workload
  profile** — for W2 that is `tx on` with `tso/gso/gro/lro off`, not all-off;
  a DUT offload outside its workload profile voids the run;
- workload: id, parameters, seeds, duration, run index;
- data: raw samples + computed median/p95/p99 + bootstrap CI half-width.

Reproducibility contract: any run re-executable from its manifest with the
same seeds and the pinned QEMU/kernel/rootfs.

## 8. Preregister the guardrail band (gate before any comparison)

Per `02` §4: from the native W2/W3 distributions compute the native-side 95%
CI, then **commit the headroom multiple `k` here, in writing, before any
Regseq number exists**:

> Preregistered band (filled in at pilot close, 2026-__-__):
> k = ____ ; W2-throughput band = ____ ; W2-CPU/byte band = ____ ;
> W3-RTT band = ____ . "Pathological regression" ≡ Regseq median outside this
> band. No post-hoc retune.

**Immutability.** When this block is filled, it is committed to git in the
same commit as the native-pilot manifests; that commit hash is recorded in the
campaign archive and the comparison campaign cites it. The band is fixed at
that commit. Any later change is a separate, explicitly-labeled amendment
commit with rationale — never an in-place edit — so "no post-hoc retune" is
enforced by version control, not honor system.

The comparison campaign may not begin until this block is filled **and
committed**, and the smoke gate (§4) and W1–W3 native collection (§5) have
passed.

## 9. What this runbook is NOT

Not the Regseq-vs-native comparison (later campaign, same harness, only the
bound driver changes). Not a performance microbenchmark. No QEMU device-model
patch (`03` §3). No absolute-hardware or codegen claim (`01` §4).

## 10. Change history

- 2026-05-19: Authored. Realizes `02` §7 for the x86_64+KVM native pilot;
  obeys the no-device-model-patch invariant; W4 link-flap uses patch-free QMP
  `set_link`.
