# Agentic Code Reasoning — Actionable Playbook

> Extracted from *Agentic Code Reasoning* (Ugare & Chandra, Meta, 2026). A practitioner's reference for building LLM agents that reason about code semantics without executing it.

---

## 1. Core Idea: Semi-formal Reasoning

Standard chain-of-thought lets an agent make unsupported claims about code behavior. **Semi-formal reasoning** forces the agent to fill in a structured "certificate" — explicit premises, traced execution paths, and a formal conclusion — before it can answer. The format acts as a checklist: the agent cannot skip cases or guess.

**Why it works:** Structuring the *reasoning process* (not just the output format) forces the agent to gather evidence before concluding. This naturally produces deeper interprocedural reasoning because tracing program paths requires following function calls rather than guessing their behavior.

**Tradeoff:** Semi-formal mode uses roughly 2–3x more agent steps than standard reasoning, but reduces errors by nearly half.

---

## 2. Design Principles

1. **Structure the reasoning, not just the output.** A JSON schema for the answer isn't enough. The template must force intermediate evidence-gathering steps (premises, traces, counterexamples).

2. **Require explicit premises.** Every claim must trace back to a specific code location (file:line). This prevents pattern-matching on function names.

3. **Force counterexample search.** If claiming equivalence/correctness, the agent must argue why no counterexample exists. If claiming a difference, it must produce one.

4. **Enforce per-test / per-region analysis.** Don't let the agent make a blanket judgment. Make it analyze each test case or code region individually.

5. **Use hypothesis-driven exploration.** When the agent reads a file, it should state what it expects to find and why *before* reading, then update its hypothesis after.

6. **Task-specific templates generalize better than task-specific training.** The same LLM with different prompt templates handled patch equivalence, fault localization, and code QA — no fine-tuning needed.

7. **Agentic > single-shot.** Letting the agent explore the codebase with tools consistently outperforms dumping all context into a single prompt, because relevant context often spans multiple files and dependencies.

---

## 3. Reusable Templates

### 3.1 Patch Equivalence Verification

Use when: determining whether two code patches produce the same behavior (e.g., verifying an LLM-generated patch against a reference solution).

```
Semi-formal Proof of Patch Equivalence

DEFINITIONS:
D1: Two patches are EQUIVALENT MODULO TESTS iff executing the
    existing repository test suite produces identical pass/fail
    outcomes for both patches.
D2: The relevant tests are ONLY those in FAIL_TO_PASS and
    PASS_TO_PASS (the existing test suite in the repository).

PREMISES (state what each patch does):
P1: Patch 1 modifies [file(s)] by [specific change description]
P2: Patch 2 modifies [file(s)] by [specific change description]
P3: The FAIL_TO_PASS tests check [specific behavior being tested]
P4: The PASS_TO_PASS tests check [specific behavior, if relevant]

ANALYSIS OF TEST BEHAVIOR:
For FAIL_TO_PASS test(s):
  Claim 1.1: With Patch 1 applied, test [name] will [PASS/FAIL]
             because [trace through the code behavior]
  Claim 1.2: With Patch 2 applied, test [name] will [PASS/FAIL]
             because [trace through the code behavior]
  Comparison: [SAME/DIFFERENT] outcome

For PASS_TO_PASS test(s) (if patches could affect them differently):
  Claim 2.1: With Patch 1 applied, test behavior is [description]
  Claim 2.2: With Patch 2 applied, test behavior is [description]
  Comparison: [SAME/DIFFERENT] outcome

EDGE CASES RELEVANT TO EXISTING TESTS:
(Only analyze edge cases that the ACTUAL tests exercise)
E1: [Edge case that existing tests exercise]
  - Patch 1 behavior: [specific output/behavior]
  - Patch 2 behavior: [specific output/behavior]
  - Test outcome same: [YES/NO]

COUNTEREXAMPLE (required if claiming NOT EQUIVALENT):
Test [name] will [PASS/FAIL] with Patch 1 because [reason]
Test [name] will [FAIL/PASS] with Patch 2 because [reason]
Therefore patches produce DIFFERENT test outcomes.

OR

NO COUNTEREXAMPLE EXISTS (required if claiming EQUIVALENT):
All existing tests produce identical outcomes because [reason]

FORMAL CONCLUSION:
By Definition D1:
- Test outcomes with Patch 1: [PASS/FAIL for each test]
- Test outcomes with Patch 2: [PASS/FAIL for each test]
- Since test outcomes are [IDENTICAL/DIFFERENT], patches are
  [EQUIVALENT/NOT EQUIVALENT] modulo the existing tests.

ANSWER: [YES/NO]
```

### 3.2 Fault Localization

Use when: given a failing test and a codebase, identify the buggy lines of code (no stack traces or error messages provided).

```
Semi-formal Fault Localization Template

## Phase 1: Test Semantics Analysis
- What does the failing test method do step by step?
- What are the explicit assertions / expected exceptions?
- What is the expected behavior vs. the observed failure mode?
- State these as formal PREMISES:
  PREMISE T1: The test calls X.method(args) and expects [behavior]
  PREMISE T2: The test asserts [condition]
  ...

## Phase 2: Code Path Tracing
- Trace the execution path from the test's entry point into
  production code
- For each significant method call, document:
  METHOD: ClassName.methodName(params)
  LOCATION: file:line
  BEHAVIOR: what this method does
  RELEVANT: why it matters to the test
- Build a call sequence showing the flow from test -> production code

## Phase 3: Divergence Analysis
- For each code path traced, identify where the implementation
  could diverge from the test's expectations
- State divergences as formal claims:
  CLAIM D1: At [file:line], [code] would produce [behavior]
            which contradicts PREMISE T[N] because [reason]
  CLAIM D2: ...
- Each claim must reference a specific PREMISE and a specific
  code location

## Phase 4: Ranked Predictions
- Based on the divergence claims, produce ranked predictions
- Each prediction must cite the supporting CLAIM(s)
```

**Structured exploration format** (use during agentic file navigation):

```
### When requesting a file:

HYPOTHESIS H[N]: [What you expect to find and why it may
                  contain the bug]
EVIDENCE: [What from the test or previously read files
           supports this hypothesis]
CONFIDENCE: [high/medium/low]

### After reading a file:

OBSERVATIONS from [filename]:
  O[N]: [Key observation about the code, with line numbers]
  O[N]: [Another observation]

HYPOTHESIS UPDATE:
  H[M]: [CONFIRMED | REFUTED | REFINED] - [Explanation]

UNRESOLVED:
  - [What questions remain unanswered]
  - [What other files/functions might need examination]

NEXT ACTION RATIONALE: [Why reading another file, or why
                        enough evidence to predict]
```

### 3.3 Code Question Answering

Use when: answering nuanced questions about code behavior, library semantics, or project-specific logic.

```
Semi-formal Reasoning Template

FUNCTION TRACE TABLE:
| Function/Method | File:Line | Parameter Types | Return Type | Behavior (VERIFIED) |
| --------------- | --------- | --------------- | ----------- | ------------------- |
| [function1]     | [file:N]  | [param types]   | [ret type]  | [ACTUAL behavior]   |

DATA FLOW ANALYSIS:
Variable: [key variable name]
- Created at: [file:line]
- Modified at: [file:line(s), or 'NEVER MODIFIED']
- Used at: [file:line(s)]

SEMANTIC PROPERTIES:
Property 1: [e.g., 'HashMap is mutable']
- Evidence: [specific file:line]

ALTERNATIVE HYPOTHESIS CHECK:
If the opposite answer were true, what evidence would exist?
- Searched for: [what you looked for]
- Found: [what you found - cite file:line]
- Conclusion: [REFUTED / SUPPORTED]

<answer>[Final answer with explicit evidence]</answer>
```

---

## 4. Common Failure Modes to Guard Against

These are the error patterns observed even with semi-formal reasoning. Build your prompts and review processes to watch for them.

### 4.1 Patch Equivalence Failures

| Failure Mode                      | Description                                                                                       | Mitigation                                                                  |
| --------------------------------- | ------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| **Incomplete execution tracing**  | Agent assumes function behavior without fully tracing concrete execution paths                    | Require the agent to show the full call chain for each claim                |
| **Third-party library guessing**  | Agent guesses behavior from function names when source code is unavailable                        | Instruct agent to flag when source is unavailable and reason conservatively |
| **Dismissing subtle differences** | Agent identifies a semantic difference but incorrectly concludes it's irrelevant to test outcomes | Require per-test impact analysis for every identified difference            |

### 4.2 Fault Localization Failures

| Failure Mode             | Description                                                                                                  | Mitigation                                                                                    |
| ------------------------ | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------- |
| **Indirection bugs**     | Bug is in a class not directly invoked by the test (e.g., test calls `parse()` but bug is in a config class) | Trace at least 2 levels of indirection from the test entry point                              |
| **Multi-file bugs**      | Bugs spanning 2+ files are systematically harder                                                             | Explicitly prompt the agent to consider cross-file interactions                               |
| **Domain-specific bugs** | Algorithmic issues (e.g., numerical analysis) exceed the model's domain expertise                            | Acknowledge confidence limits; suggest domain-specific static analysis tools                  |
| **Stopping at symptoms** | Agent identifies the crash site but not the root cause                                                       | Require the PREMISE → CLAIM → PREDICTION chain so the agent traces back from symptom to cause |

### 4.3 Code QA Failures

| Failure Mode                     | Description                                                                            | Mitigation                                                                                     |
| -------------------------------- | -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| **Confident wrong answers**      | Deep reasoning follows a plausible-but-incomplete chain, missing downstream code paths | Require the alternative hypothesis check; force the agent to argue for the opposite conclusion |
| **Guessing from function names** | Agent infers behavior from naming conventions instead of reading the implementation    | Require the function trace table with verified behavior and file:line citations                |

---

## 5. Prompting Strategies

### 5.1 How to Structure a Verification Agent Prompt

1. **Give the agent tools, not just context.** Let it use bash to navigate files, search for definitions, grep for usages. Agentic exploration consistently outperforms stuffing everything into a single prompt.

2. **Include the template in the system/initial prompt.** The certificate template should be part of the agent's instructions from the start, not requested after it has already begun reasoning.

3. **Set a generous step budget.** Semi-formal reasoning used ~28–43 steps on average vs. ~10–20 for standard mode. Cap at ~100 steps to prevent runaway exploration.

4. **Disable execution and git.** The paper's setup intentionally prevents code execution and git history access. This forces genuine semantic reasoning rather than shortcutting via `git log` or running the tests.

### 5.2 When to Use Each Template

| Task                                                     | Template                 | Key Signal                                 |
| -------------------------------------------------------- | ------------------------ | ------------------------------------------ |
| "Are these two patches equivalent?"                      | Patch Equivalence (3.1)  | Two diffs + a test suite                   |
| "Where is the bug?"                                      | Fault Localization (3.2) | A failing test + a codebase                |
| "What does this code do?" / "Is X true about this code?" | Code QA (3.3)            | A question about code behavior             |
| Code review / PR verification                            | Adapt Patch Equivalence  | Compare PR diff against expected behavior  |
| RL reward signal (execution-free)                        | Patch Equivalence (3.1)  | Comparing generated patch to gold solution |

### 5.3 Adapting Templates to New Tasks

The paper suggests semi-formal reasoning can extend to security vulnerability detection, code smell identification, and API misuse detection. To adapt:

1. **Define the formal property** you're checking (analogous to D1 in patch equivalence).
2. **Specify what premises the agent must state** (what the code does, what the standard/spec requires).
3. **Require traced evidence** — the agent must follow call chains and cite file:line.
4. **Require a counterexample or proof of absence** for the final conclusion.
5. **Add an alternative hypothesis check** to counteract confirmation bias.

---

## 6. Architecture Notes

- **Agent framework:** Minimal SWE-agent setup with bash tool access, max 100 steps.
- **No sandbox needed:** The agent cannot execute repository code — dependencies are not installed. It *can* run independent Python scripts to probe language behavior (e.g., testing regex edge cases).
- **Model choice matters:** Opus-4.5 benefits more from the structured template than Sonnet-4.5, likely because the template corrects Opus's tendency to guess from function names, while Sonnet already reasons more carefully by default.
- **Cost/latency tradeoff:** Semi-formal uses ~2.8x the steps. For batch/offline verification (e.g., RL reward signals), this is fine. For real-time code review, consider using it selectively on high-risk diffs.

---

## 7. Practical Applications

1. **Execution-free RL reward signals.** Use the patch equivalence template to verify LLM-generated patches against gold solutions without setting up execution environments. Achieves 93% accuracy — potentially reliable enough for training signal.

2. **Automated code review.** Adapt the templates for PR review: treat the PR diff as Patch 1 and the expected behavior (from the issue/spec) as the reference.

3. **Bug triage / fault localization.** Given a failing test and no stack trace, the fault localization template guides systematic investigation.

4. **Static analysis replacement.** Instead of building language-specific analysis tools, prompt an LLM with task-specific reasoning templates that generalize across languages and frameworks.

5. **Verification in CI pipelines.** Use as a lightweight check before committing to expensive test execution — flag patches that are likely to fail.