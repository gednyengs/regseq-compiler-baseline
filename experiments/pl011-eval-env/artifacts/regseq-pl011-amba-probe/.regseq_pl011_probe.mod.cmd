savedcmd_regseq_pl011_probe.mod := printf '%s\n'   regseq_pl011_probe.o | awk '!x[$$0]++ { print("./"$$0) }' > regseq_pl011_probe.mod
