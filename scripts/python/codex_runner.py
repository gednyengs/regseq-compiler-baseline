#!/usr/bin/env python3
"""Invoke the Codex CLI non-interactively and return structured results.

Codex is invoked via `codex exec --json`, which streams JSONL events to
stdout. The runner consumes that stream, keeps only the fields that
matter for downstream processing, and returns a :class:`CodexResult`
(or prints one as JSON in CLI mode).

Library usage::

    from codex_runner import run_codex
    result = run_codex(prompt, timeout=900)
    print(result.final_message)

CLI usage::

    cat prompt.txt | uv run python scripts/python/codex_runner.py \\
        --model gpt-5.4 --effort high --timeout 900 > result.json

The script reads the prompt from stdin. All flags are optional; defaults
match the review pattern used by the Regseq harness (gpt-5.4, high
effort, yolo mode, 15 minute timeout).

Prompt convention (GPT-5.4 prompting style -- agents craft prompts
themselves; this runner is task-agnostic)::

    <task>
      <concrete job and the files/context it applies to>
    </task>

    <grounding_rules>
      Cite file:line for every claim. Do not speculate about code you
      did not read. Mark hypotheses as hypotheses.
    </grounding_rules>

    <structured_output_contract>
      First line: VERDICT: <APPROVED|REQUIRES FIXES>
      If REQUIRES FIXES: numbered list, each entry exactly
        <file>:<line-range> -- <what is wrong> -- <what is required>
      Total response <= N words.
    </structured_output_contract>

    <default_follow_through_policy>
      Do not ask clarifying questions. Read the files, decide, stop.
    </default_follow_through_policy>

Agents composing reviews SHOULD follow this shape so downstream parsing
of `VERDICT:` lines and fix lists stays consistent, but the runner
itself does not enforce it.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import subprocess
import sys
from pathlib import Path


@dataclasses.dataclass
class CodexResult:
    """Structured result from a single ``codex exec`` invocation."""

    final_message: str
    session_id: str
    input_tokens: int
    cached_input_tokens: int
    output_tokens: int
    exit_code: int


class CodexError(Exception):
    """Raised when Codex fails to produce a usable result."""


def run_codex(
    prompt: str,
    *,
    model: str = "gpt-5.4",
    effort: str = "high",
    yolo: bool = True,
    cwd: Path | None = None,
    timeout: int = 900,
) -> CodexResult:
    """Invoke ``codex exec --json`` with *prompt* on stdin and return a :class:`CodexResult`.

    Parameters:
        prompt: Natural-language prompt passed to Codex on stdin.
        model: Model name for ``-m``. Defaults to ``gpt-5.4``.
        effort: Reasoning effort (``low`` | ``medium`` | ``high`` | ``xhigh``).
            Applied via ``-c model_reasoning_effort=<effort>``.
        yolo: When True, adds ``--dangerously-bypass-approvals-and-sandbox``
            so Codex can run shell commands without prompting. The Regseq
            harness runs in a trusted local workspace; set False only if
            you are exposing the runner to untrusted input.
        cwd: Working directory for Codex (``-C``). Defaults to the
            current process cwd.
        timeout: Wall-clock timeout in seconds. Raised as :class:`CodexError`.

    Returns:
        A :class:`CodexResult` with the final agent message plus usage
        metadata.

    Raises:
        CodexError: If Codex is not on PATH, times out, or emits no
            ``agent_message`` event.
    """
    cmd: list[str] = [
        "codex",
        "exec",
        "--json",
        "-m",
        model,
        "-c",
        f"model_reasoning_effort={effort}",
    ]
    if yolo:
        cmd.append("--dangerously-bypass-approvals-and-sandbox")
    if cwd is not None:
        cmd.extend(["-C", str(cwd)])
    cmd.append("-")  # read prompt from stdin

    try:
        proc = subprocess.run(
            cmd,
            input=prompt,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as exc:
        raise CodexError(f"codex exec timed out after {timeout}s") from exc
    except FileNotFoundError as exc:
        raise CodexError("codex CLI not found on PATH") from exc

    final_message: str | None = None
    session_id: str = ""
    input_tokens = 0
    cached_input_tokens = 0
    output_tokens = 0

    for raw in proc.stdout.splitlines():
        line = raw.strip()
        # Codex interleaves a small amount of plain-text framing (e.g.
        # "Reading additional input from stdin...") with the JSONL
        # stream; filter by "starts with {" before json.loads.
        if not line.startswith("{"):
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        etype = event.get("type", "")
        if etype == "thread.started":
            session_id = event.get("thread_id", "")
        elif etype == "item.completed":
            item = event.get("item", {}) or {}
            if item.get("type") == "agent_message":
                # Multiple agent_message events can appear in one turn;
                # last one wins (that is Codex's final answer).
                final_message = item.get("text", "")
        elif etype == "turn.completed":
            usage = event.get("usage", {}) or {}
            input_tokens = int(usage.get("input_tokens", 0) or 0)
            cached_input_tokens = int(usage.get("cached_input_tokens", 0) or 0)
            output_tokens = int(usage.get("output_tokens", 0) or 0)

    if final_message is None:
        stderr_tail = (proc.stderr or "")[-500:]
        raise CodexError(
            "codex produced no agent_message "
            f"(exit={proc.returncode}, stderr_tail={stderr_tail!r})"
        )

    return CodexResult(
        final_message=final_message,
        session_id=session_id,
        input_tokens=input_tokens,
        cached_input_tokens=cached_input_tokens,
        output_tokens=output_tokens,
        exit_code=proc.returncode,
    )


def _build_cli() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Invoke Codex non-interactively. Prompt is read from stdin. "
            "Result is printed as a JSON object on stdout."
        ),
    )
    parser.add_argument("--model", default="gpt-5.4", help="Codex model (default: gpt-5.4)")
    parser.add_argument(
        "--effort",
        default="high",
        choices=["none", "minimal", "low", "medium", "high", "xhigh"],
        help="Reasoning effort (default: high)",
    )
    parser.add_argument(
        "--cwd",
        type=Path,
        default=None,
        help="Working directory for codex (default: current shell cwd)",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=900,
        help="Wall-clock timeout in seconds (default: 900)",
    )
    parser.add_argument(
        "--no-yolo",
        action="store_true",
        help="Do NOT pass --dangerously-bypass-approvals-and-sandbox",
    )
    parser.add_argument(
        "--message-only",
        action="store_true",
        help="Print only the final agent message (no JSON wrapper)",
    )
    return parser


def _main(argv: list[str]) -> int:
    args = _build_cli().parse_args(argv)
    prompt = sys.stdin.read()
    if not prompt.strip():
        print("error: empty prompt on stdin", file=sys.stderr)
        return 2
    try:
        result = run_codex(
            prompt,
            model=args.model,
            effort=args.effort,
            yolo=not args.no_yolo,
            cwd=args.cwd,
            timeout=args.timeout,
        )
    except CodexError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    if args.message_only:
        sys.stdout.write(result.final_message)
        if not result.final_message.endswith("\n"):
            sys.stdout.write("\n")
    else:
        json.dump(dataclasses.asdict(result), sys.stdout, indent=2)
        sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(_main(sys.argv[1:]))
