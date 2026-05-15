# AGENTS.md

## 1. Coding Guidelines

### 1.1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 1.2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions invented for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Simple is not unstructured. If the domain has an accepted way to factor, name, or represent the problem (a visitor over an AST, a reducer for state, a pipeline for stream processing), use it. The rule forbids unrequested complexity, not domain idioms - those are how practitioners read code.

Ask: "Would a senior engineer say this is overcomplicated?" If yes, simplify.
Also ask: "Would a senior engineer in this domain recognize the shape?" If no, you may have stripped away structure that's load-bearing.

### 1.3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 1.4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

### 1.5. Verify Before Claiming Done

**Don't report success you haven't verified.**

- Type-checks, compilation, and "no errors" are not behavior.
- If the user asked for behavior, exercise the behavior.
- If you can't verify (no runtime, no test environment), say so explicitly - don't imply success.
- "Tests pass" only counts if the tests actually cover what was asked.

The test: Could you demo this to the user right now and have it work?

## 2. Running Codex

Codex is invoked non-interactively through `scripts/python/codex_runner.py`.
The runner wraps `codex exec --json`, consumes the JSONL event stream, and
returns a structured result. Call it via the Bash tool; do not use any
agent-based or skill-based wrapper around Codex.

### 2.1. Invocation

The runner reads the prompt from **stdin** only. There is no flag for
passing the prompt as a string or file. Pipe it in with a heredoc:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py [OPTIONS]
<prompt body>
EOF
```

Set the Bash tool's own `timeout` parameter to at least the runner's
`--timeout` value so the shell call does not kill Codex mid-run.

### 2.2. Command-line options

| Flag                | Values                                                        | Default       | Purpose                                                                                                     |
| ------------------- | ------------------------------------------------------------- | ------------- | ----------------------------------------------------------------------------------------------------------- |
| `--model MODEL`     | string                                                        | `gpt-5.5`     | Model passed to `codex exec -m`                                                                             |
| `--effort LEVEL`    | `none` \| `minimal` \| `low` \| `medium` \| `high` \| `xhigh` | `high`        | Sets `model_reasoning_effort`                                                                               |
| `--cwd PATH`        | path                                                          | process cwd   | Working directory for Codex (`-C`)                                                                          |
| `--timeout SECONDS` | int                                                           | `900`         | Wall-clock timeout; exceeding it raises an error                                                            |
| `--no-yolo`         | flag                                                          | off (yolo on) | Omits `--dangerously-bypass-approvals-and-sandbox`; use when Codex should not run shell commands unattended |
| `--message-only`    | flag                                                          | off           | Print only `final_message` text instead of the JSON wrapper                                                 |
| `-h`, `--help`      | flag                                                          | -             | Show argparse help                                                                                          |

There are no positional arguments.

### 2.3. Output

- **Default**: pretty-printed JSON (`indent=2`) on stdout with fields
  `final_message`, `session_id`, `input_tokens`, `cached_input_tokens`,
  `output_tokens`, `exit_code`.
- **With `--message-only`**: the raw `final_message` text, newline-terminated.
- **Errors**: written to stderr, prefixed with `error: `.

### 2.4. Exit codes

- `0` - Codex produced a final `agent_message`.
- `1` - Codex failure: CLI not on PATH, timeout, or no `agent_message` in the event stream.
- `2` - Empty prompt on stdin.

### 2.5. Prompt shape (recommended, not enforced)

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

### 2.6. Library use

The runner is also importable:

```python
from codex_runner import run_codex, CodexResult, CodexError

result = run_codex(
    prompt,
    model="gpt-5.5",
    effort="high",
    yolo=True,
    cwd=None,
    timeout=900,
)
print(result.final_message)
```

`run_codex` raises `CodexError` if the `codex` CLI is missing from PATH,
the call times out, or the event stream contains no `agent_message`.

### 2.7. Iteration pattern

Each `run_codex` call is independent; there is no session continuation.
When a response indicates more work is needed, act on it locally, then
re-invoke the runner with a fresh prompt that reflects the new state.
Do not try to resume a prior Codex turn.
