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
        ("query_operational_state", interface.ops.query_operational_state),
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
                .ok_or_else(|| EmitError::Driver(format!("network operation `{name}` does not resolve to an instance")))
        })
        .collect()
}

pub(super) fn container_fields(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    if !model.network_interface.drivers.contains_key(&driver) {
        return String::new();
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

static int {drv}_net_change_mtu(struct net_device *dev, int mtu)
{{
	WRITE_ONCE(dev->mtu, mtu);
	return 0;
}}

static int {drv}_net_set_mac_address(struct net_device *dev, void *address)
{{
	return eth_mac_addr(dev, address);
}}

static void {drv}_net_set_rx_mode(struct net_device *dev)
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
    out.push_str("/* No per-packet completion callback is emitted: terminal observation is absent. */\n\n");
    out
}

pub(super) fn probe_prepare(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    if !model.network_interface.drivers.contains_key(&driver) {
        return String::new();
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
    let Some(interface) = model.network_interface.drivers.get(&driver) else {
        return String::new();
    };
    if interface.ops.set_published.is_none() {
        return "\t/* runtime publication capability absent: interface remains backend-provided only */\n".into();
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
    format!(
        "\tif (priv->tx_ring)\n\t\tdma_free_coherent(&pdev->dev, {}_TX_RING_SIZE * sizeof(*priv->tx_ring), priv->tx_ring, priv->tx_ring_dma);\n\tif (priv->netdev)\n\t\tfree_netdev(priv->netdev);\n",
        drv.to_ascii_uppercase(),
    )
}

pub(super) fn remove(model: &RegseqModel, driver: ItemId, drv: &str) -> String {
    let Some(interface) = model.network_interface.drivers.get(&driver) else {
        return String::new();
    };
    let withdraw = interface.ops.set_published.is_some().then_some("\tunregister_netdev(priv->netdev);\n").unwrap_or("");
    format!(
        r#"	/* NIM teardown: deferred/IRQ drains precede this withdrawal and free. */
	netif_tx_disable(priv->netdev);
	synchronize_net();
	(void){drv}_net_complete_tx(priv, {upper}_TX_RING_SIZE);
{withdraw}	dma_free_coherent(&pdev->dev,
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
