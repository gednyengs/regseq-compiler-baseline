//! Linux lowering for the locked network-interface element.
//!
//! The netdev surface is complete (admin, packet handoff, carrier, identity,
//! configuration, counters, and optional observers). Descriptor layout,
//! producer/consumer indices, mappings, IRQ dispatch, and deferred work stay
//! native L2 state and never enter the portable network table.

use regseq_model::{InstanceId, InstanceRef, ItemId, RegseqModel};

use super::body::LowerCx;
use super::EmitError;

const RING_SIZE: usize = 64;
const E1000_RING_SIZE: usize = 256;

pub(super) fn descriptor_types(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    if !super::dma::descriptor_datapath(model, driver) {
        return String::new();
    }
    let upper = drv.to_ascii_uppercase();
    format!(
        r#"/* Intel 82540EM legacy layouts: rx_tx_operations.md 47-100, 487-555. */
#define {upper}_DESCRIPTOR_RING_SIZE 256u
#define {upper}_RX_BUFFER_SIZE 2048u
#define {upper}_TX_DOORBELL_BATCH 6u

struct {drv}_legacy_tx_descriptor {{
	u64 buffer_address;
	u16 length;
	u8 checksum_offset;
	u8 command;
	volatile u8 status;
	u8 checksum_start;
	u16 special;
}} __packed;

struct {drv}_legacy_rx_descriptor {{
	u64 buffer_address;
	volatile u16 length;
	volatile u16 checksum;
	volatile u8 status;
	volatile u8 errors;
	volatile u16 special;
}} __packed;

static_assert(sizeof(struct {drv}_legacy_tx_descriptor) == 16);
static_assert(sizeof(struct {drv}_legacy_rx_descriptor) == 16);

struct {drv}_priv;
static bool {drv}_schedule_napi(struct {drv}_priv *priv, bool irqoff);

"#
    )
}

pub(super) fn roots(
    model: &RegseqModel,
    driver: ItemId,
) -> Result<Vec<(&'static str, InstanceId)>, EmitError> {
    let Some(interface) = model.network_interface.drivers.get(&driver) else {
        return Ok(Vec::new());
    };
    let refs: [(&str, Option<InstanceRef>); 12] = [
        ("set_published", interface.ops.set_published),
        ("set_admin_enabled", interface.ops.set_admin_enabled),
        ("transmit", interface.ops.transmit),
        ("receive", interface.ops.receive),
        ("report_carrier", interface.ops.report_carrier),
        ("report_dormant", interface.ops.report_dormant),
        (
            "query_operational_state",
            interface.ops.query_operational_state,
        ),
        ("get_property", interface.ops.get_property),
        ("set_property", interface.ops.set_property),
        ("read_counters", interface.ops.read_counters),
        ("register_observer", interface.ops.register_observer),
        ("unregister_observer", interface.ops.unregister_observer),
    ];
    refs.into_iter()
        .filter_map(|(name, reference)| reference.map(|reference| (name, reference)))
        .map(|(name, reference)| {
            resolve_instance_ref(model, reference)
                .map(|id| (name, id))
                .ok_or_else(|| {
                    EmitError::Driver(format!(
                        "network operation `{name}` does not resolve to an instance"
                    ))
                })
        })
        .collect()
}

pub(super) fn container_fields(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    if !model.network_interface.drivers.contains_key(&driver) {
        return String::new();
    }
    if super::dma::descriptor_datapath(model, driver) {
        return format!(
            "\tstruct net_device *netdev;\n\tstruct napi_struct napi;\n\tbool __rseq_napi_enabled;\n\tstruct delayed_work __rseq_link_work;\n\tu64 __rseq_itr_window_tx_packets;\n\tu64 __rseq_itr_window_rx_packets;\n\tu64 __rseq_itr_window_tx_bytes;\n\tu64 __rseq_itr_window_rx_bytes;\n\tstruct sk_buff *__rseq_tx_packets[{E1000_RING_SIZE}];\n\tdma_addr_t __rseq_tx_dma[{E1000_RING_SIZE}];\n\tu32 __rseq_tx_dma_len[{E1000_RING_SIZE}];\n\tu8 __rseq_tx_dma_page[{E1000_RING_SIZE}];\n\tu8 __rseq_tx_descriptor_count[{E1000_RING_SIZE}];\n\tu8 __rseq_tx_advanced[{E1000_RING_SIZE}];\n\tstruct sk_buff *__rseq_rx_packets[{E1000_RING_SIZE}];\n\tdma_addr_t __rseq_rx_packet_dma[{E1000_RING_SIZE}];\n"
        );
    }
    format!(
        "\tstruct net_device *netdev;\n\tstruct {drv}_tx_descriptor *tx_ring;\n\tdma_addr_t tx_ring_dma;\n\tu16 tx_producer;\n\tu16 tx_consumer;\n\tstruct sk_buff *tx_packets[{RING_SIZE}];\n\tdma_addr_t tx_packet_dma[{RING_SIZE}];\n"
    )
}

pub(super) fn emit(cx: &mut LowerCx<'_>, driver: ItemId, drv: &str) -> String {
    let Some(interface) = cx.model.network_interface.drivers.get(&driver) else {
        return String::new();
    };
    for include in [
        "<linux/dma-mapping.h>",
        "<linux/etherdevice.h>",
        "<linux/netdevice.h>",
        "<linux/skbuff.h>",
    ] {
        cx.extra_includes.insert(include);
    }
    if interface.ops.register_observer.is_some() || interface.ops.unregister_observer.is_some() {
        cx.extra_includes.insert("<linux/notifier.h>");
    }
    if super::dma::descriptor_datapath(cx.model, driver) {
        return emit_descriptor_interface(cx, interface, drv);
    }

    let mut out = format!(
        r#"/* RSEQ-NIM: one net_device; packet/ring/DMA/IRQ/work mechanisms stay L2. */
#define {drv_upper}_TX_RING_SIZE {ring}u

struct {drv}_tx_descriptor {{
	dma_addr_t address;
	u32 length;
	u32 flags;
}};

static struct {drv}_priv *{drv}_network_priv(struct net_device *dev)
{{
	return *(struct {drv}_priv **)netdev_priv(dev);
}}

static int {drv}_net_open(struct net_device *dev)
{{
	netif_start_queue(dev);
	return 0;
}}

static int {drv}_net_stop(struct net_device *dev)
{{
	netif_tx_disable(dev);
	return 0;
}}

static netdev_tx_t {drv}_net_transmit(struct sk_buff *packet,
		struct net_device *dev)
{{
	struct {drv}_priv *priv = {drv}_network_priv(dev);
	u16 slot = priv->tx_producer;
	u16 next = (u16)((slot + 1u) % {drv_upper}_TX_RING_SIZE);
	dma_addr_t address;

	if (next == READ_ONCE(priv->tx_consumer)) {{
		netif_stop_queue(dev);
		return NETDEV_TX_BUSY; /* rejection: caller retains packet */
	}}
	address = dma_map_single(&priv->pdev->dev, packet->data, packet->len,
		DMA_TO_DEVICE);
	if (dma_mapping_error(&priv->pdev->dev, address)) {{
		dev_kfree_skb_any(packet);
		dev->stats.tx_dropped++;
		return NETDEV_TX_OK;
	}}
	priv->tx_packets[slot] = packet;
	priv->tx_packet_dma[slot] = address;
	priv->tx_ring[slot].address = address;
	priv->tx_ring[slot].length = packet->len;
	priv->tx_ring[slot].flags = 1u;
	dma_wmb();
	WRITE_ONCE(priv->tx_producer, next);
	return NETDEV_TX_OK; /* accepted: binding owns reclamation */
}}

static unsigned int {drv}_net_complete_tx(struct {drv}_priv *priv,
		unsigned int budget)
{{
	unsigned int done = 0;
	while (priv->tx_consumer != READ_ONCE(priv->tx_producer) && done < budget) {{
		u16 slot = priv->tx_consumer;
		struct sk_buff *packet = priv->tx_packets[slot];
		dma_unmap_single(&priv->pdev->dev, priv->tx_packet_dma[slot],
			packet->len, DMA_TO_DEVICE);
		priv->netdev->stats.tx_packets++;
		priv->netdev->stats.tx_bytes += packet->len;
		priv->tx_packets[slot] = NULL;
		dev_consume_skb_any(packet);
		priv->tx_consumer = (u16)((slot + 1u) % {drv_upper}_TX_RING_SIZE);
		done++;
	}}
	if (netif_queue_stopped(priv->netdev) &&
	    ((priv->tx_producer + 1u) % {drv_upper}_TX_RING_SIZE) != priv->tx_consumer)
		netif_wake_queue(priv->netdev);
	return done;
}}

static int __maybe_unused {drv}_net_receive(struct {drv}_priv *priv, struct sk_buff *packet)
{{
	packet->dev = priv->netdev;
	return netif_rx(packet); /* no normalized L1 receive verdict */
}}

static void __maybe_unused {drv}_net_report_carrier(struct {drv}_priv *priv, bool present)
{{
	if (present)
		netif_carrier_on(priv->netdev);
	else
		netif_carrier_off(priv->netdev);
}}

static void {drv}_net_get_stats64(struct net_device *dev,
		struct rtnl_link_stats64 *stats)
{{
	stats->rx_packets = dev->stats.rx_packets;
	stats->tx_packets = dev->stats.tx_packets;
	stats->rx_bytes = dev->stats.rx_bytes;
	stats->tx_bytes = dev->stats.tx_bytes;
	stats->rx_errors = dev->stats.rx_errors;
	stats->tx_errors = dev->stats.tx_errors;
	stats->rx_dropped = dev->stats.rx_dropped;
	stats->tx_dropped = dev->stats.tx_dropped;
}}

static int __maybe_unused {drv}_net_change_mtu(struct net_device *dev, int mtu)
{{
	WRITE_ONCE(dev->mtu, mtu);
	return 0;
}}

static int __maybe_unused {drv}_net_set_mac_address(struct net_device *dev, void *address)
{{
	return eth_mac_addr(dev, address);
}}

static void __maybe_unused {drv}_net_set_rx_mode(struct net_device *dev)
{{
	(void)dev; /* filter programming remains provider-specific L2 state */
}}

static const struct net_device_ops {drv}_netdev_ops = {{
	.ndo_open = {drv}_net_open,
	.ndo_stop = {drv}_net_stop,
	.ndo_start_xmit = {drv}_net_transmit,
	.ndo_get_stats64 = {drv}_net_get_stats64,
"#,
        drv_upper = drv.to_ascii_uppercase(),
        ring = RING_SIZE,
    );
    if interface.ops.set_property.is_some() {
        out.push_str(&format!(
            "\t.ndo_change_mtu = {drv}_net_change_mtu,\n\t.ndo_set_mac_address = {drv}_net_set_mac_address,\n\t.ndo_set_rx_mode = {drv}_net_set_rx_mode,\n"
        ));
    }
    out.push_str("};\n\n");
    out.push_str(&format!(
        "/* Native identity/property sources: dev->name, dev->ifindex, dev->mtu,\n * dev->dev_addr, carrier state, and rtnl_link_stats64. */\n"
    ));
    for capability in interface.ops.capabilities() {
        out.push_str(&format!("/* RSEQ-NIM-CAPABILITY {capability} */\n"));
    }
    out.push_str(
        "/* No per-packet completion callback is emitted: terminal observation is absent. */\n\n",
    );
    out
}

fn emit_descriptor_interface(
    cx: &mut LowerCx<'_>,
    interface: &regseq_model::DriverNetworkInterface,
    drv: &str,
) -> String {
    cx.extra_includes.insert("<linux/workqueue.h>");
    let set_admin = op(cx, interface.ops.set_admin_enabled).expect("set_admin_enabled");
    let transmit = op(cx, interface.ops.transmit).expect("transmit");
    let counters = op(cx, interface.ops.read_counters).expect("read_counters");
    let upper = drv.to_ascii_uppercase();
    let mut out = format!(
        r#"/* RSEQ-NIM/E1000: the DMA-construct rings are the only hardware rings. */
static struct {drv}_priv *{drv}_network_priv(struct net_device *dev)
{{
	return *(struct {drv}_priv **)netdev_priv(dev);
}}

static unsigned int {drv}_tx_desc_unused(struct {drv}_priv *priv)
{{
	unsigned int head = READ_ONCE(priv->inner.tx_head);
	unsigned int tail = READ_ONCE(priv->inner.tx_tail);

	return head > tail ? head - tail - 1u :
		{upper}_DESCRIPTOR_RING_SIZE - tail + head - 1u;
}}

/* Linux e1000_main.c:3066-3095: stop, publish the stop, then recheck. */
static int {drv}_maybe_stop_tx(struct {drv}_priv *priv, unsigned int needed)
{{
	if (likely({drv}_tx_desc_unused(priv) >= needed))
		return 0;
	netif_stop_queue(priv->netdev);
	smp_mb();
	if (likely({drv}_tx_desc_unused(priv) < needed))
		return -EBUSY;
	netif_start_queue(priv->netdev);
	return 0;
}}

static unsigned int {drv}_tx_desc_needed(struct sk_buff *packet)
{{
	unsigned int data = (skb_headlen(packet) ? 1u : 0u) +
		skb_shinfo(packet)->nr_frags;

	return data + (packet->ip_summed == CHECKSUM_PARTIAL ? 1u : 0u);
}}

/* Linux e1000_main.c:3875-3894: pair reclaim publication with queue wake. */
static void {drv}_maybe_wake_tx(struct {drv}_priv *priv, u32 tx_done)
{{
	if (unlikely(tx_done && netif_carrier_ok(priv->netdev) &&
	    {drv}_tx_desc_unused(priv) >= 32u)) {{
		smp_mb();
		if (netif_queue_stopped(priv->netdev) &&
		    READ_ONCE(priv->inner.admin_enabled))
			netif_wake_queue(priv->netdev);
	}}
}}

/* Linux e1000_main.c:3776-3787: one counter window per NAPI ownership. */
static bool {drv}_schedule_napi(struct {drv}_priv *priv, bool irqoff)
{{
	if (napi_schedule_prep(&priv->napi)) {{
		priv->__rseq_itr_window_tx_packets = 0;
		priv->__rseq_itr_window_rx_packets = 0;
		priv->__rseq_itr_window_tx_bytes = 0;
		priv->__rseq_itr_window_rx_bytes = 0;
		if (irqoff)
			__napi_schedule_irqoff(&priv->napi);
		else
			__napi_schedule(&priv->napi);
		return true;
	}}
	if (READ_ONCE(priv->inner.admin_enabled))
		writel(0x00d5u, priv->inner.regs + {upper}_IMS);
	return false;
}}

/* Linux e1000_main.c:2424-2551, 3765-3769: LSC owns a watchdog path.
 * The explicit NAPI kick mirrors the stock watchdog's ICS_RXDMT0 service
 * kick (2543-2544) without making link recovery depend on a data IRQ. */
static void {drv}_link_work(struct work_struct *work)
{{
	struct {drv}_priv *priv = container_of(to_delayed_work(work),
		struct {drv}_priv, __rseq_link_work);
	u32 status;
	bool carrier;

	if (!READ_ONCE(priv->inner.admin_enabled))
		return;
	status = readl(priv->inner.regs + {upper}_STATUS);
	carrier = (status & 0x2u) != 0;
	{drv}_report_carrier(&priv->inner, carrier);
	if (!carrier) {{
		netif_carrier_off(priv->netdev);
		schedule_delayed_work(&priv->__rseq_link_work, 2 * HZ);
		return;
	}}
	netif_carrier_on(priv->netdev);
	/* Linux e1000_main.c:381-397 wakes the TX queue before its LSC-driven
	 * watchdog pass; 2441-2478 publishes the carrier-up transition.  TX
	 * reclaim may have completed while carrier was down, so repeat the ring
	 * space wake here without requiring a fresh completion. */
	if (unlikely({drv}_tx_desc_unused(priv) >= 32u)) {{
		smp_mb();
		if (netif_queue_stopped(priv->netdev) &&
		    READ_ONCE(priv->inner.admin_enabled))
			netif_wake_queue(priv->netdev);
	}}
	writel(0xffffffffu, priv->inner.regs + {upper}_IMC);
	(void)readl(priv->inner.regs + {upper}_STATUS);
	(void){drv}_schedule_napi(priv, false);
}}

static int {drv}_net_open(struct net_device *dev)
{{
	struct {drv}_priv *priv = {drv}_network_priv(dev);
	int ret;

	if (!priv->__rseq_napi_enabled) {{
		napi_enable(&priv->napi);
		priv->__rseq_napi_enabled = true;
	}}
	ret = (int){set_admin}(&priv->inner, true);
	if (ret && priv->__rseq_napi_enabled) {{
		napi_disable(&priv->napi);
		priv->__rseq_napi_enabled = false;
	}}
	if (!ret)
		netif_start_queue(dev);
	return ret;
}}

static int {drv}_net_stop(struct net_device *dev)
{{
	struct {drv}_priv *priv = {drv}_network_priv(dev);
	int ret;

	netif_tx_disable(dev);
	ret = (int){set_admin}(&priv->inner, false);
	cancel_delayed_work_sync(&priv->__rseq_link_work);
	if (priv->__rseq_napi_enabled) {{
		napi_disable(&priv->napi);
		priv->__rseq_napi_enabled = false;
	}}
	return ret;
}}

static netdev_tx_t {drv}_net_transmit(struct sk_buff *packet,
		struct net_device *dev)
{{
	struct {drv}_priv *priv = {drv}_network_priv(dev);
	struct OpaquePacketBuffer native = {{ .identity = (size_t)(uintptr_t)packet }};
	struct Option_2 verdict;
	unsigned int needed = {drv}_tx_desc_needed(packet);

	/* Linux e1000_main.c:3197-3208: admit the packet's descriptor count,
	 * retaining the stock two-descriptor head/tail safety gap. */
	if (unlikely({drv}_maybe_stop_tx(priv, needed + 2u)))
		return NETDEV_TX_BUSY;
	verdict = {transmit}(&priv->inner, native);

	if (verdict._raw[0] == 0) {{
		/* Linux e1000_main.c:3248-3271: reserve the next worst-case send. */
		(void){drv}_maybe_stop_tx(priv, MAX_SKB_FRAGS + 7u);
		return NETDEV_TX_OK; /* accepted: descriptor ring owns packet */
	}}
	(void){drv}_maybe_stop_tx(priv, needed + 2u);
	return NETDEV_TX_BUSY; /* rejected: caller retains packet */
}}

static int __maybe_unused {drv}_net_receive(struct {drv}_priv *priv,
		struct sk_buff *packet)
{{
	packet->dev = priv->netdev;
	packet->protocol = eth_type_trans(packet, priv->netdev);
	return (int)napi_gro_receive(&priv->napi, packet);
}}

/* Linux NAPI realizes the authored IRQ -> budgeted TX+RX service contract.
 * The constants mirror rich-e1000.rs device facts; NAPI remains below L1. */
static int {drv}_napi_poll(struct napi_struct *napi, int budget)
{{
	struct {drv}_priv *priv = container_of(napi, struct {drv}_priv, napi);
	int work_budget;
	u32 status;
	u32 tx_done;
	u32 rx_done;
	u32 tx_packets;
	u32 rx_packets;
	u32 tx_bytes;
	u32 rx_bytes;
	bool carrier;

	if (budget <= 0) {{
		tx_done = {drv}_harvest_tx(&priv->inner, 64u);
		{drv}_maybe_wake_tx(priv, tx_done);
		return 0;
	}}
	work_budget = min_t(int, budget, 64);
	status = readl(priv->inner.regs + {upper}_STATUS);
	carrier = (status & 0x2u) != 0;
	{drv}_report_carrier(&priv->inner, carrier);
	if (carrier)
		netif_carrier_on(priv->netdev);
	else
		netif_carrier_off(priv->netdev);
	tx_done = {drv}_harvest_tx(&priv->inner, work_budget);
	rx_done = {drv}_harvest_rx(&priv->inner, work_budget);
	{drv}_maybe_wake_tx(priv, tx_done);
	if (tx_done == work_budget || rx_done == work_budget)
		return budget;
	if (napi_complete_done(napi, rx_done)) {{
		tx_packets = (u32)priv->__rseq_itr_window_tx_packets;
		rx_packets = (u32)priv->__rseq_itr_window_rx_packets;
		tx_bytes = (u32)priv->__rseq_itr_window_tx_bytes;
		rx_bytes = (u32)priv->__rseq_itr_window_rx_bytes;
		{drv}_set_itr(&priv->inner, tx_packets, tx_bytes, rx_packets, rx_bytes);
		if (READ_ONCE(priv->inner.admin_enabled))
			writel(0x00d5u, priv->inner.regs + {upper}_IMS);
	}}
	return rx_done;
}}

static void __maybe_unused {drv}_net_report_carrier(struct {drv}_priv *priv,
		bool present)
{{
	if (present)
		netif_carrier_on(priv->netdev);
	else
		netif_carrier_off(priv->netdev);
}}

static void {drv}_net_get_stats64(struct net_device *dev,
		struct rtnl_link_stats64 *stats)
{{
	struct {drv}_priv *priv = {drv}_network_priv(dev);
	struct InterfaceCounters counters = {counters}(&priv->inner);
	stats->rx_packets = counters.rx_packets;
	stats->tx_packets = counters.tx_packets;
	stats->rx_bytes = counters.rx_bytes;
	stats->tx_bytes = counters.tx_bytes;
	stats->rx_errors = counters.rx_errors;
	stats->tx_errors = counters.tx_errors;
	stats->rx_dropped = counters.rx_dropped;
	stats->tx_dropped = counters.tx_dropped;
}}

static int {drv}_net_change_mtu(struct net_device *dev, int mtu)
{{
	struct {drv}_priv *priv = {drv}_network_priv(dev);
	struct NetworkProperty property = {{ ._raw = {{ 6 }} }};
	struct NetworkPropertyValue value = {{0}};
	int ret;

	value._raw[0] = 1; /* NetworkPropertyValue::U32 */
	memcpy(&value._raw[4], &mtu, sizeof(u32));
	ret = (int){drv}_set_property(&priv->inner, property, value);
	if (!ret)
		WRITE_ONCE(dev->mtu, mtu);
	return ret;
}}

static int {drv}_net_set_mac_address(struct net_device *dev, void *opaque)
{{
	struct {drv}_priv *priv = {drv}_network_priv(dev);
	struct sockaddr *address = opaque;
	struct LinkLayerAddress native = {{
		.length = ETH_ALEN,
		.octet0 = address->sa_data[0], .octet1 = address->sa_data[1],
		.octet2 = address->sa_data[2], .octet3 = address->sa_data[3],
		.octet4 = address->sa_data[4], .octet5 = address->sa_data[5],
	}};
	int ret = (int){drv}_program_mac(&priv->inner, native);
	if (!ret)
		ret = eth_mac_addr(dev, opaque);
	return ret;
}}

static void {drv}_net_set_rx_mode(struct net_device *dev)
{{
	struct {drv}_priv *priv = {drv}_network_priv(dev);
	u32 mode = dev->flags & IFF_PROMISC ? 2u :
		(dev->flags & IFF_ALLMULTI ? 1u : 0u);
	WRITE_ONCE(priv->inner.filter_mode, mode);
	writel({drv}_receive_control(&priv->inner), priv->inner.regs + {upper}_RCTL);
}}

static const struct net_device_ops {drv}_netdev_ops = {{
	.ndo_open = {drv}_net_open,
	.ndo_stop = {drv}_net_stop,
	.ndo_start_xmit = {drv}_net_transmit,
	.ndo_get_stats64 = {drv}_net_get_stats64,
	.ndo_change_mtu = {drv}_net_change_mtu,
	.ndo_set_mac_address = {drv}_net_set_mac_address,
	.ndo_set_rx_mode = {drv}_net_set_rx_mode,
}};

"#,
    );
    for capability in interface.ops.capabilities() {
        out.push_str(&format!("/* RSEQ-NIM-CAPABILITY {capability} */\n"));
    }
    out.push_str("/* Packet completion is harvested from the device DD bit; no completion is fabricated. */\n\n");
    out
}

fn op(cx: &LowerCx<'_>, reference: Option<InstanceRef>) -> Option<String> {
    let reference = reference?;
    let id = resolve_instance_ref(cx.model, reference)?;
    cx.fn_symbols.get(&id).cloned()
}

pub(super) fn probe_prepare(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    if !model.network_interface.drivers.contains_key(&driver) {
        return String::new();
    }
    if super::dma::descriptor_datapath(model, driver) {
        return format!(
            r#"
	/* NIM prepare: native publication state precedes authored DMA allocation. */
	priv->netdev = alloc_etherdev(sizeof(struct {drv}_priv *));
	if (!priv->netdev)
		return -ENOMEM;
	*(struct {drv}_priv **)netdev_priv(priv->netdev) = priv;
	SET_NETDEV_DEV(priv->netdev, &pdev->dev);
	priv->netdev->netdev_ops = &{drv}_netdev_ops;
	priv->netdev->min_mtu = 68;
	priv->netdev->max_mtu = 9000;
	priv->netdev->hw_features = NETIF_F_HW_CSUM | NETIF_F_SG;
	priv->netdev->vlan_features = NETIF_F_HW_CSUM | NETIF_F_SG;
	priv->netdev->features |= NETIF_F_HW_CSUM | NETIF_F_SG;
	priv->netdev->features &= ~(NETIF_F_RXCSUM |
		NETIF_F_TSO | NETIF_F_TSO6);
	INIT_DELAYED_WORK(&priv->__rseq_link_work, {drv}_link_work);
	netif_napi_add(priv->netdev, &priv->napi, {drv}_napi_poll);
	napi_enable(&priv->napi);
	priv->__rseq_napi_enabled = true;
	netif_carrier_off(priv->netdev);
	netif_tx_stop_all_queues(priv->netdev);
"#
        );
    }
    format!(
        r#"
	/* NIM prepare: all native state exists before the publication boundary. */
	priv->netdev = alloc_etherdev(sizeof(struct {drv}_priv *));
	if (!priv->netdev)
		return -ENOMEM;
	*(struct {drv}_priv **)netdev_priv(priv->netdev) = priv;
	SET_NETDEV_DEV(priv->netdev, &pdev->dev);
	priv->netdev->netdev_ops = &{drv}_netdev_ops;
	priv->netdev->min_mtu = 68;
	priv->netdev->max_mtu = 9000;
	/* alloc_etherdev installed the kernel default ethtool_ops; the authored
	 * surface has no ethtool-specific capability, so no custom table is set. */
	netif_carrier_off(priv->netdev);
	netif_tx_stop_all_queues(priv->netdev);
	priv->tx_ring = dma_alloc_coherent(&pdev->dev,
		{upper}_TX_RING_SIZE * sizeof(*priv->tx_ring),
		&priv->tx_ring_dma, GFP_KERNEL);
	if (!priv->tx_ring) {{
		free_netdev(priv->netdev);
		priv->netdev = NULL;
		return -ENOMEM;
	}}
"#,
        upper = drv.to_ascii_uppercase(),
    )
}

pub(super) fn probe_publish(
    model: &RegseqModel,
    driver: ItemId,
    drv: &str,
    failure_cleanup: &str,
) -> String {
    let Some(_interface) = model.network_interface.drivers.get(&driver) else {
        return String::new();
    };
    if super::dma::descriptor_datapath(model, driver) {
        return format!(
            r#"
	/* RAR0 was synchronized into authored state before this visibility boundary. */
	{{
		u8 __mac[ETH_ALEN] = {{
			(u8)priv->inner.mac_low, (u8)(priv->inner.mac_low >> 8),
			(u8)(priv->inner.mac_low >> 16), (u8)(priv->inner.mac_low >> 24),
			(u8)priv->inner.mac_high, (u8)(priv->inner.mac_high >> 8),
		}};
		eth_hw_addr_set(priv->netdev, __mac);
	}}
	{{
		int __publish = register_netdev(priv->netdev);
		if (__publish) {{
{failure_cleanup}
			free_netdev(priv->netdev);
			priv->netdev = NULL;
			return __publish;
		}}
	}}
"#,
            failure_cleanup = failure_cleanup.replace('\t', "\t\t\t"),
        );
    }
    format!(
        r#"
	/* Publication is the visibility boundary; consumers may act after success. */
	{{
		int __publish = register_netdev(priv->netdev);
		if (__publish) {{
{failure_cleanup}
			dma_free_coherent(&pdev->dev,
				{upper}_TX_RING_SIZE * sizeof(*priv->tx_ring),
				priv->tx_ring, priv->tx_ring_dma);
			free_netdev(priv->netdev);
			priv->netdev = NULL;
			return __publish;
		}}
	}}
"#,
        upper = drv.to_ascii_uppercase(),
        failure_cleanup = failure_cleanup.replace('\t', "\t\t\t"),
    )
}

pub(super) fn probe_failure(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    if !model.network_interface.drivers.contains_key(&driver) {
        return String::new();
    }
    if super::dma::descriptor_datapath(model, driver) {
        return "\tif (priv->netdev) {\n\t\tif (priv->__rseq_napi_enabled) {\n\t\t\tnapi_disable(&priv->napi);\n\t\t\tpriv->__rseq_napi_enabled = false;\n\t\t}\n\t\tnetif_napi_del(&priv->napi);\n\t\tfree_netdev(priv->netdev);\n\t}\n".into();
    }
    format!(
        "\tif (priv->tx_ring)\n\t\tdma_free_coherent(&pdev->dev, {}_TX_RING_SIZE * sizeof(*priv->tx_ring), priv->tx_ring, priv->tx_ring_dma);\n\tif (priv->netdev)\n\t\tfree_netdev(priv->netdev);\n",
        drv.to_ascii_uppercase(),
    )
}

pub(super) fn pre_teardown(model: &RegseqModel, driver: ItemId) -> String {
    if !super::dma::descriptor_datapath(model, driver) {
        return String::new();
    }
    r#"	/* Stop new stack traffic and drain NAPI before authored ring release. */
	netif_tx_disable(priv->netdev);
	cancel_delayed_work_sync(&priv->__rseq_link_work);
	if (priv->__rseq_napi_enabled) {
		napi_disable(&priv->napi);
		priv->__rseq_napi_enabled = false;
	}
"#
    .into()
}

pub(super) fn remove(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    let Some(_interface) = model.network_interface.drivers.get(&driver) else {
        return String::new();
    };
    if super::dma::descriptor_datapath(model, driver) {
        return format!(
            r#"	/* DMA rings were released by authored on_deinit before withdrawal. */
	synchronize_net();
	unregister_netdev(priv->netdev);
	netif_napi_del(&priv->napi);
	free_netdev(priv->netdev);
	priv->netdev = NULL;
"#
        );
    }
    format!(
        r#"	/* NIM teardown: deferred/IRQ drains precede this withdrawal and free. */
	netif_tx_disable(priv->netdev);
	synchronize_net();
	(void){drv}_net_complete_tx(priv, {upper}_TX_RING_SIZE);
	unregister_netdev(priv->netdev);
	dma_free_coherent(&pdev->dev,
		{upper}_TX_RING_SIZE * sizeof(*priv->tx_ring),
		priv->tx_ring, priv->tx_ring_dma);
	free_netdev(priv->netdev);
	priv->netdev = NULL;
"#,
        upper = drv.to_ascii_uppercase(),
    )
}

fn resolve_instance_ref(model: &RegseqModel, target: InstanceRef) -> Option<InstanceId> {
    (0..model.instances.len())
        .map(|i| InstanceId::from_u32(i as u32))
        .find(|id| model.instance_ref_of(*id) == target)
}
