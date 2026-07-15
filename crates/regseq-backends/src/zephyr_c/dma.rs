//! Locked asynchronous DMA channel lowering for Zephyr.
//!
//! Zephyr has no owned-buffer allocator in the locked surface, so any
//! `DmaBuffer` element is rejected. The channel path uses the public DMA API;
//! endpoint/controller resolution remains an L2 parameter.

use regseq_model::{ItemId, RegseqModel};

use super::body::LowerCx;
use super::EmitError;

pub(super) fn emit(
    cx: &mut LowerCx<'_>,
    driver: ItemId,
    drv: &str,
) -> Result<String, EmitError> {
    let Some(dma) = cx.model.dma.drivers.get(&driver) else {
        return Ok(String::new());
    };
    if dma.buffer.is_some() {
        return Err(EmitError::Rejected(
            "DmaBuffer owned allocation is unsupported on Zephyr: the locked headers expose caller-buffer alignment but no owned-buffer allocator".into(),
        ));
    }
    let Some(channel) = &dma.channel else {
        return Ok(String::new());
    };
    cx.extra_includes.insert("<errno.h>");
    cx.extra_includes.insert("<stdbool.h>");
    cx.extra_includes.insert("<string.h>");
    cx.extra_includes.insert("<zephyr/drivers/dma.h>");

    let ops = &channel.ops;
    let mut out = format!(
        r#"/* RSEQ-DMA-CHANNEL: controller resolution is L2; channel ownership is exclusive. */
struct {drv}_dma_channel {{
	const struct device *controller;
	uint32_t channel;
	struct dma_block_config block;
	struct dma_config config;
	bool configured;
	bool prepared;
	bool submitted;
	bool active;
	bool terminated;
	bool synchronized;
}};

static int __unused {drv}_dma_channel_acquire(const struct device *controller,
		void *filter_param,
		struct {drv}_dma_channel *owned)
{{
	int channel;
	if (!controller || !owned)
		return -EINVAL;
	channel = dma_request_channel(controller, filter_param);
	if (channel < 0)
		return channel;
	memset(owned, 0, sizeof(*owned));
	owned->controller = controller;
	owned->channel = (uint32_t)channel;
	return 0;
}}

static int __unused {drv}_dma_channel_configure(struct {drv}_dma_channel *owned,
		enum dma_channel_direction direction, uintptr_t source,
		uintptr_t destination, size_t length)
{{
	int ret;

	if (!owned || !owned->controller || !length || owned->prepared ||
	    owned->submitted || owned->active)
		return -EINVAL;
	owned->configured = false;
	memset(&owned->block, 0, sizeof(owned->block));
	memset(&owned->config, 0, sizeof(owned->config));
	owned->block.source_address = source;
	owned->block.dest_address = destination;
	owned->block.block_size = length;
	owned->config.channel_direction = direction;
	owned->config.block_count = 1;
	owned->config.head_block = &owned->block;
	owned->prepared = false;
	owned->submitted = false;
	owned->active = false;
	owned->terminated = false;
	owned->synchronized = false;
	ret = dma_config(owned->controller, owned->channel, &owned->config);
	if (!ret)
		owned->configured = true;
	return ret;
}}

static int __unused {drv}_dma_channel_prepare(struct {drv}_dma_channel *owned)
{{
	if (!owned || !owned->controller || !owned->configured || owned->prepared ||
	    owned->submitted || owned->active)
		return -EINVAL;
	owned->prepared = true;
	return 0;
}}

static int __unused {drv}_dma_channel_submit(struct {drv}_dma_channel *owned)
{{
	if (!owned || !owned->prepared)
		return -EINVAL;
	owned->prepared = false;
	owned->submitted = true;
	return 0;
}}

static int __unused {drv}_dma_channel_activate(struct {drv}_dma_channel *owned)
{{
	int ret;

	if (!owned || !owned->submitted)
		return -EINVAL;
	ret = dma_start(owned->controller, owned->channel);
	if (!ret) {{ owned->submitted = false; owned->active = true; }}
	return ret;
}}

"#
    );
    if ops.status.is_some() {
        out.push_str(&format!("static int __unused {drv}_dma_channel_status(struct {drv}_dma_channel *owned, struct dma_status *status)\n{{\n\tint ret;\n\tif (!owned || !owned->controller || !status)\n\t\treturn -EINVAL;\n\tret = dma_get_status(owned->controller, owned->channel, status);\n\tif (!ret && !status->busy)\n\t\towned->active = false;\n\treturn ret;\n}}\n\n"));
    }
    if ops.pause.is_some() || ops.suspend.is_some() {
        out.push_str(&format!("static int __unused {drv}_dma_channel_suspend(struct {drv}_dma_channel *owned)\n{{\n\treturn owned && owned->controller ? dma_suspend(owned->controller, owned->channel) : -EINVAL;\n}}\n\n"));
    }
    if ops.resume.is_some() {
        out.push_str(&format!("static int __unused {drv}_dma_channel_resume(struct {drv}_dma_channel *owned)\n{{\n\treturn owned && owned->controller ? dma_resume(owned->controller, owned->channel) : -EINVAL;\n}}\n\n"));
    }
    if ops.terminate.is_some() {
        out.push_str(&format!("static int __unused {drv}_dma_channel_terminate(struct {drv}_dma_channel *owned)\n{{\n\tint ret;\n\tif (!owned || !owned->controller)\n\t\treturn -EINVAL;\n\tret = dma_stop(owned->controller, owned->channel);\n\tif (!ret) {{ owned->submitted = false; owned->active = false; owned->terminated = true; owned->synchronized = false; }}\n\treturn ret; /* no completion callback is synthesized */\n}}\n\n"));
    }
    if ops.synchronize.is_some() {
        out.push_str(&format!("static int __unused {drv}_dma_channel_synchronize(struct {drv}_dma_channel *owned)\n{{\n\tif (!owned || !owned->terminated)\n\t\treturn -EINVAL;\n\t/* Zephyr dma_stop is the terminal barrier; preserve the separate L1 step. */\n\towned->synchronized = true;\n\treturn 0;\n}}\n\n"));
    }
    out.push_str(&format!("static int __unused {drv}_dma_channel_release(struct {drv}_dma_channel *owned)\n{{\n\tif (!owned || !owned->controller)\n\t\treturn -EINVAL;\n\tif (owned->prepared || owned->submitted || owned->active ||\n\t    (owned->terminated && !owned->synchronized))\n\t\treturn -EBUSY;\n\tdma_release_channel(owned->controller, owned->channel);\n\towned->controller = NULL;\n\treturn 0;\n}}\n\n"));
    for capability in ops.capabilities() {
        out.push_str(&format!("/* RSEQ-DMA-CHANNEL-CAPABILITY {capability} */\n"));
    }
    out.push_str("/* Missing status/control/callback operations remain unsupported; termination emits no completion. */\n\n");
    Ok(out)
}

#[allow(dead_code)]
fn _model_anchor(_: &RegseqModel) {}
