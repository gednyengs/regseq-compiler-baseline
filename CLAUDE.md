# CLAUDE.md

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

## 2. External Review

Codex (GPT-5.5) is the project's external review model; invocation conventions live in the `codex-runner` skill.
