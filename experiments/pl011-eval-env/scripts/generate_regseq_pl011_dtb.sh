#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <input-dtb> <output-dtb>" >&2
  exit 1
fi

input_dtb="$1"
output_dtb="$2"
tmp_dts="$(mktemp)"
trap 'rm -f "$tmp_dts"' EXIT

dtc -I dtb -O dts "$input_dtb" > "$tmp_dts"

python3 - "$tmp_dts" <<'PY'
import pathlib
import re
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text()

pattern = re.compile(
    r'(pl011@9040000\s*\{\n)'
    r'((?:[^\n]*\n)*?)'
    r'(\s*compatible\s*=\s*"arm,pl011\\0arm,primecell";\n)',
    re.MULTILINE,
)

replacement = (
    r'\1'
    r'\2'
    r'\t\tarm,primecell-periphid = <0x00abc123>;\n'
    r'\t\tcompatible = "regseq,pl011\\0arm,primecell";\n'
)

new_text, count = pattern.subn(replacement, text, count=1)
if count != 1:
    raise SystemExit("failed to locate pl011@9040000 node for DT update")

path.write_text(new_text)
PY

dtc -I dts -O dtb "$tmp_dts" -o "$output_dtb"
echo "wrote $output_dtb"
