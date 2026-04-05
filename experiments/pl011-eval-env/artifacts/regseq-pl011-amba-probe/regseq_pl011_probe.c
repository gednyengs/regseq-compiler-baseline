// SPDX-License-Identifier: GPL-2.0-only
#include <linux/amba/bus.h>
#include <linux/init.h>
#include <linux/ioport.h>
#include <linux/module.h>
#include <linux/of.h>

#define REGSEQ_PL011_TEST_ID 0x00abc123
#define REGSEQ_PL011_TEST_MASK 0x00ffffff

static int regseq_pl011_probe(struct amba_device *dev, const struct amba_id *id)
{
	const struct resource *res = &dev->res;
	struct device_node *node = dev->dev.of_node;

	dev_info(&dev->dev,
		 "regseq probe matched: periphid=0x%08x id=0x%08x mask=0x%08x base=%pa size=0x%llx irq0=%u\n",
		 dev->periphid,
		 id ? id->id : 0,
		 id ? id->mask : 0,
		 &res->start,
		 (unsigned long long)resource_size(res),
		 dev->irq[0]);

	if (node) {
		dev_info(&dev->dev, "device-tree node: %pOF\n", node);
	}

	return 0;
}

static void regseq_pl011_remove(struct amba_device *dev)
{
	dev_info(&dev->dev, "regseq probe driver removed\n");
}

static const struct amba_id regseq_pl011_ids[] = {
	{
		.id = REGSEQ_PL011_TEST_ID,
		.mask = REGSEQ_PL011_TEST_MASK,
	},
	{ 0, 0 },
};
MODULE_DEVICE_TABLE(amba, regseq_pl011_ids);

static const struct of_device_id regseq_pl011_of_match[] = {
	{ .compatible = "regseq,pl011" },
	{ }
};
MODULE_DEVICE_TABLE(of, regseq_pl011_of_match);

static struct amba_driver regseq_pl011_driver = {
	.drv = {
		.name = "regseq-pl011-probe",
		.of_match_table = regseq_pl011_of_match,
	},
	.id_table = regseq_pl011_ids,
	.probe = regseq_pl011_probe,
	.remove = regseq_pl011_remove,
};

module_amba_driver(regseq_pl011_driver);

MODULE_AUTHOR("OpenAI Codex");
MODULE_DESCRIPTION("Minimal AMBA probe driver for regseq PL011 DT/periphid test");
MODULE_LICENSE("GPL");
