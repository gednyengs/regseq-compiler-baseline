# CLAUDE.md

## Running Codex

Codex is invoked non-interactively through `scripts/python/codex_runner.py`.
The runner wraps `codex exec --json`, consumes the JSONL event stream, and
returns a structured result. Call it via the Bash tool; do not use any
agent-based or skill-based wrapper around Codex.

### Invocation

The runner reads the prompt from **stdin** only. There is no flag for
passing the prompt as a string or file. Pipe it in with a heredoc:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py [OPTIONS]
<prompt body>
EOF
```

Set the Bash tool's own `timeout` parameter to at least the runner's
`--timeout` value so the shell call does not kill Codex mid-run.

### Command-line options

| Flag                | Values                                                        | Default       | Purpose                                                                                                     |
| ------------------- | ------------------------------------------------------------- | ------------- | ----------------------------------------------------------------------------------------------------------- |
| `--model MODEL`     | string                                                        | `gpt-5.4`     | Model passed to `codex exec -m`                                                                             |
| `--effort LEVEL`    | `none` \| `minimal` \| `low` \| `medium` \| `high` \| `xhigh` | `high`        | Sets `model_reasoning_effort`                                                                               |
| `--cwd PATH`        | path                                                          | process cwd   | Working directory for Codex (`-C`)                                                                          |
| `--timeout SECONDS` | int                                                           | `900`         | Wall-clock timeout; exceeding it raises an error                                                            |
| `--no-yolo`         | flag                                                          | off (yolo on) | Omits `--dangerously-bypass-approvals-and-sandbox`; use when Codex should not run shell commands unattended |
| `--message-only`    | flag                                                          | off           | Print only `final_message` text instead of the JSON wrapper                                                 |
| `-h`, `--help`      | flag                                                          | -             | Show argparse help                                                                                          |

There are no positional arguments.

### Output

- **Default**: pretty-printed JSON (`indent=2`) on stdout with fields
  `final_message`, `session_id`, `input_tokens`, `cached_input_tokens`,
  `output_tokens`, `exit_code`.
- **With `--message-only`**: the raw `final_message` text, newline-terminated.
- **Errors**: written to stderr, prefixed with `error: `.

### Exit codes

- `0` - Codex produced a final `agent_message`.
- `1` - Codex failure: CLI not on PATH, timeout, or no `agent_message` in the event stream.
- `2` - Empty prompt on stdin.

### Prompt shape (recommended, not enforced)

Codex is one-shot per invocation and cannot ask follow-up questions, so
the prompt should be self-contained and tell Codex exactly what output
shape you will parse. A four-block structure keeps prompts consistent
and downstream parsing simple:

```
<task>
  Concrete job, scope boundaries, and the files or context it applies to.
  State what is out of scope explicitly.
</task>

<grounding_rules>
  Cite file:line for every claim and every required change.
  Do not speculate about code that was not read.
  Mark hypotheses as hypotheses.
</grounding_rules>

<structured_output_contract>
  Exact response shape to emit (first-line verdict, numbered list format,
  word cap, etc.). The tighter this is, the less post-processing the
  caller needs.
</structured_output_contract>

<default_follow_through_policy>
  Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

The runner does not parse or enforce this shape; it is a convention that
keeps callers' parsing code stable across invocations.

### Library use

The runner is also importable:

```python
from codex_runner import run_codex, CodexResult, CodexError

result = run_codex(
    prompt,
    model="gpt-5.4",
    effort="high",
    yolo=True,
    cwd=None,
    timeout=900,
)
print(result.final_message)
```

`run_codex` raises `CodexError` if the `codex` CLI is missing from PATH,
the call times out, or the event stream contains no `agent_message`.

### Iteration pattern

Each `run_codex` call is independent; there is no session continuation.
When a response indicates more work is needed, act on it locally, then
re-invoke the runner with a fresh prompt that reflects the new state.
Do not try to resume a prior Codex turn.
