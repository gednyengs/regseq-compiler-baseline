# regseq-backends-03: C-AST Move + Phase-7C Subset Expansion

**Depends on**: `regseq-model-60`.
**Parallel with**: `regseq-backends-02`, `regseq-model-61`,
`regseq-model-62`, `track-a-validation-11`.

## Description

Move `crates/regseq-backends/src/linux/c_ast.rs` to a new sibling
module `crates/regseq-backends/src/c/` and expand it from the
current "top-level pretty-printing DSL" into a real C AST that
supports the subset Phase-7C CFG emission requires: statements,
expressions, initializers + designators, declarators + types, and
labels + `goto`.

Rationale is captured in migration.md Q8: the current `c_ast.rs`
models only file scaffolding (includes, struct decls, function
shells with `Vec<String>` bodies). Phase-7C goto-based CFG emission
cannot be implemented on top of a string-formatted body; it needs
structured statement / expression / label / goto nodes.

The move + expansion is scoped to the **Phase-7C urgent subset**
per migration.md Q8. `for` / `do while` / `break` / `continue` /
`switch` / `case` / `typedef` / `enum` / `union` / preprocessor
conditionals are explicitly deferred (current PL011 uses only
`while`; deferred constructs land when a future driver needs them).

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q8 (the gap
  table, the "C standard is a guardrail, not a target" principle,
  the recommended Phase-7C subset).
- `crates/regseq-backends/src/linux/c_ast.rs` lines 1-247 -- the
  current module being moved and expanded.
- `crates/regseq-backends/src/linux/emit_semantic.rs` lines
  363-950 -- the current body emission that string-formats
  statements and expressions. This task's new AST must cover
  every construct those formatters currently produce, plus the
  goto/label constructs Phase-8.1 (now 7C) requires.
- `crates/regseq-backends/src/linux/emit_module.rs` lines 99-1055
  -- string-formatted initializers and declarators that the
  expanded AST will replace when tasks 9/10 land.
- External references (used as guardrails per migration.md Q8):
  - ISO C11 Annex A.2.3 "Statements" and A.2.4 "External
    definitions" for production naming.
  - `lang-c` (`docs.rs/lang-c/latest/lang_c/ast/`) for a
    Rust-flavored reference AST shape.
  - ANTLR `grammars-v4/c/CParser.g4` for grammar checks.

## Implementation Notes

### Module move

Create `crates/regseq-backends/src/c/mod.rs` with the relocated
contents of `linux/c_ast.rs`. Update
`crates/regseq-backends/src/lib.rs` to add `pub mod c;`. Update
every Linux backend import site to use `crate::c::*` instead of
`crate::linux::c_ast::*` (wait for tasks 9/10 to actually migrate
emitter call sites; this task only makes the new module available
and keeps `linux::c_ast` as a `pub use crate::c::*;` re-export to
avoid breaking current consumers).

Retain `generated_file_banner` in `linux/` (relocated to a new
`linux::conventions` module or similar) as Linux-specific policy
per migration.md Q8. `include_guard_name` is generic; move it to
`backends::c`.

### New AST nodes (Phase-7C subset)

Add the following to `crates/regseq-backends/src/c/` under
logical submodules (`statements.rs`, `expressions.rs`,
`declarators.rs`, `initializers.rs`, etc., as size dictates):

**Statements** (`statements.rs`):

```rust
/// C statement (ISO C11 6.8 `statement`).
#[derive(Debug, Clone)]
pub enum CStmt {
    /// Compound statement: `{ decls... stmts... }` (6.8.2).
    Compound { items: Vec<CBlockItem> },
    /// Declaration statement (6.7).
    Decl(CDeclaration),
    /// Expression statement (6.8.3).
    Expr(CExpr),
    /// `if` / `if-else` (6.8.4.1 selection-statement).
    If {
        cond: CExpr,
        then_branch: Box<CStmt>,
        else_branch: Option<Box<CStmt>>,
    },
    /// `while` loop (6.8.5.1 iteration-statement).
    While { cond: CExpr, body: Box<CStmt> },
    /// `return expr;` or `return;` (6.8.6.4 jump-statement).
    Return(Option<CExpr>),
    /// `goto <label>;` (6.8.6.1).
    Goto(String),
    /// Labeled statement: `<label>: <stmt>` (6.8.1).
    Labeled { label: String, stmt: Box<CStmt> },
    /// Empty statement (6.8.3 / null).
    Empty,
    /// Raw C text escape hatch (retained for transition).
    Raw(String),
}

/// A compound-statement block item: either a decl or a stmt
/// (6.8.2 block-item).
#[derive(Debug, Clone)]
pub enum CBlockItem {
    Decl(CDeclaration),
    Stmt(CStmt),
}
```

**Expressions** (`expressions.rs`):

```rust
/// C expression (ISO C11 6.5 / 6.7.6).
#[derive(Debug, Clone)]
pub enum CExpr {
    /// Integer literal (6.4.4.1).
    IntLiteral { value: i128, suffix: CIntSuffix },
    /// String literal (6.4.5).
    StringLiteral(String),
    /// Identifier reference.
    Ident(String),
    /// Field projection: `<expr>.<name>` or `<expr>-><name>`.
    Field { base: Box<CExpr>, name: String, via_ptr: bool },
    /// Array index: `<expr>[<idx>]`.
    Index { base: Box<CExpr>, idx: Box<CExpr> },
    /// Function call: `<callee>(<args>, ...)`.
    Call { callee: Box<CExpr>, args: Vec<CExpr> },
    /// Unary operator.
    Unary { op: CUnaryOp, operand: Box<CExpr> },
    /// Binary operator.
    Binary { op: CBinaryOp, lhs: Box<CExpr>, rhs: Box<CExpr> },
    /// Cast: `(<type>)<expr>`.
    Cast { ty: CType, expr: Box<CExpr> },
    /// Assignment: `<lhs> = <rhs>` or compound-assign.
    Assign {
        lhs: Box<CExpr>,
        op: Option<CBinaryOp>, // None = plain =; Some = compound-assign
        rhs: Box<CExpr>,
    },
    /// Ternary: `<c> ? <t> : <f>`.
    Ternary {
        cond: Box<CExpr>,
        then_branch: Box<CExpr>,
        else_branch: Box<CExpr>,
    },
    /// Address-of / deref are UnaryOp variants.
    /// Parenthesized expression (for explicit precedence). The
    /// printer adds parens as needed by precedence; this node is
    /// only for when the authoring site wants forced parens.
    Paren(Box<CExpr>),
    /// Raw expression text escape hatch.
    Raw(String),
}
```

`CUnaryOp`, `CBinaryOp`, `CIntSuffix` defined alongside.

**Declarators + types** (`declarators.rs`):

```rust
/// C type (6.7.2 type-specifier + 6.7.3 type-qualifier).
#[derive(Debug, Clone)]
pub enum CType {
    /// Scalar type name: `int`, `u32`, `size_t`, etc.
    Named(String),
    /// Pointer: `<inner> *` (6.7.6.1).
    Pointer { inner: Box<CType>, const_qualified: bool },
    /// Array: `<inner>[<n>]` (6.7.6.2). `n` is optional.
    Array { inner: Box<CType>, size: Option<usize> },
    /// Struct reference: `struct <tag>`.
    Struct(String),
    /// Typedef reference (non-normative for this phase but supported
    /// for existing code that uses typedef names).
    Typedef(String),
}

/// C declaration (6.7).
#[derive(Debug, Clone)]
pub struct CDeclaration {
    pub storage: CStorageClass,
    pub ty: CType,
    pub declarator: CDeclarator,
    pub initializer: Option<CInitializer>,
}

pub enum CStorageClass { None, Static, Extern, /* etc. */ }

pub struct CDeclarator {
    pub name: String,
    // (function-pointer, array-of, and nested declarators handled
    // as needed by PL011 usage; document scope.)
}
```

**Initializers + designators** (`initializers.rs`):

```rust
/// C initializer (6.7.9).
#[derive(Debug, Clone)]
pub enum CInitializer {
    /// Scalar initializer: `<expr>`.
    Expr(CExpr),
    /// List initializer: `{ <item>, <item>, ... }`.
    List(Vec<CInitItem>),
}

/// A designated or positional item in a list initializer (6.7.9).
#[derive(Debug, Clone)]
pub struct CInitItem {
    pub designator: Option<CDesignator>,
    pub init: CInitializer,
}

#[derive(Debug, Clone)]
pub enum CDesignator {
    /// `.field = ...`.
    Field(String),
    /// `[idx] = ...`.
    Index(CExpr),
}
```

### Rendering

Each node type gets a `render_into(&self, out: &mut String, indent:
usize)` method (consistent with the existing `CTopLevelItem` pattern).
A top-level `render` entry point applies the normalization-algorithm
rules from task 1's contract:

- Tab indentation (match existing `c_ast.rs` convention).
- Deterministic ordering of items where ambiguous (not applicable
  for `CStmt::Compound` items, which preserve order).
- No trailing whitespace on lines.
- Exactly one blank line between top-level items.

### Integration with existing `CTopLevelItem`

`CTopLevelItem::FuncDef` currently has `body: Vec<String>`. Extend
it (or add a parallel variant) to accept `body: CStmt::Compound`
for the new CFG emitter. Keep the `Vec<String>` variant alive (not
deprecated yet) so tasks 9/10 can migrate call sites one at a time.

Variant naming: either add `CTopLevelItem::FuncDefStructured` or
rewrite `FuncDef`'s `body` field to a sum type. Implementer's
choice; document in the code.

### Linux policy re-home

`generated_file_banner` returns a `CTopLevelItem::BlockComment` with
the "SPDX-License-Identifier: GPL-2.0" + "Linux driver artifact"
text. Relocate to a new `crates/regseq-backends/src/linux/conventions.rs`
module. Update the one call site in `linux/emitter.rs:205-215` to
import from the new location. `include_guard_name` (no Linux
content) stays in `backends::c::include_guard_name`.

### What this task does NOT do

- Does not migrate `emit_semantic.rs` or `emit_module.rs` to use
  the new AST. That is tasks 9 and 10.
- Does not add statement AST nodes beyond the Phase-7C subset
  (per migration.md Q8: `for`, `do while`, `switch`, `break`,
  `continue`, `typedef`, `enum`, `union`, preprocessor conditionals
  are deferred).
- Does not relocate any Linux-specific emission logic beyond
  `generated_file_banner`.
- Does not change the `Raw(String)` escape hatch -- it remains
  available during the transition.

## Non-Goals

- No emitter-logic changes.
- No deferred C constructs.
- No C parser (we only emit).
- No migration of existing backend emission tests to use the new
  AST (tasks 9/10 handle test migration as part of emitter
  retarget).
- **Do not modify `crates/regseq-driver/src/construction*`.**

## Test Plan

- **Module-move smoke**: `cargo build -p regseq-backends` after
  relocation + re-export succeeds.
- **AST construction tests**: unit tests under
  `crates/regseq-backends/tests/c_ast_nodes.rs` that construct
  one instance of every new node type and call `render_into` on
  each, asserting plausible output shape. Not exhaustive -- the
  real validation is tasks 9/10 using the AST for full emission.
- **Integration test (small)**: construct by hand a `CStmt::Compound`
  containing: one `CDecl`, one `CStmt::Labeled` wrapping a
  `CStmt::Goto`, one `CStmt::If` with two `CStmt::Return` branches,
  one `CStmt::While` with a call expression, and one `CStmt::Return`.
  Render and assert the output parses via `cc -fsyntax-only` (wrap
  in a minimal `void f(void) { ... }` harness first).
- **Normal-form adherence**: a test that constructs the same AST
  twice, renders both, asserts byte-identical output.
- `cargo test --workspace` green (nothing behaviorally changes
  yet).

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-backends-03 against .harness/tasks/regseq-backends/03-c-ast-move-and-expand.md.

Files to audit:
- crates/regseq-backends/src/c/ (entire directory)
- crates/regseq-backends/src/linux/conventions.rs (new)
- crates/regseq-backends/src/linux/c_ast.rs (re-export shim)
- crates/regseq-backends/src/lib.rs (pub mod c;)
- crates/regseq-backends/src/linux/emitter.rs (import update for generated_file_banner)
- crates/regseq-backends/tests/c_ast_nodes.rs (new tests)

Focus on:
- Is every new AST node typed correctly per C11 Annex A production names?
  Call out any node whose naming or shape diverges from lang-c / ANTLR
  without justification.
- Does the Phase-7C subset cover every construct the current string-formatted
  emission in emit_semantic.rs and emit_module.rs produces? Specifically
  check: assignment, field projection (. and ->), call with args, binary
  bitwise ops, compound assign, cast, ternary, return with expr, return
  void, while loop with call in condition, if with early-return branches.
- Is generated_file_banner correctly relocated to linux/conventions.rs
  (Linux policy) while include_guard_name stays in backends::c (generic)?
- Is the Raw(String) escape hatch still available (transitional)?
- Are deferred constructs (for, switch, typedef, etc.) explicitly NOT
  present vs accidentally absent? Contract says they're deferred; verify.

Do NOT flag:
- Lack of emitter migration (that's tasks 9/10).
- Lack of complete C coverage (Phase-7C subset is scoped).
</task>
... (standard grounding + structured output per SESSION.md) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-backends --test c_ast_nodes
cargo test -p regseq-backends
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-backends -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-backends/src/c/` exists with `mod.rs` + submodules
  for statements / expressions / declarators / initializers.
- `crates/regseq-backends/src/lib.rs` declares `pub mod c;`.
- `crates/regseq-backends/src/linux/c_ast.rs` exists as a
  re-export shim (`pub use crate::c::*;`) so current consumers
  compile unchanged. Tasks 9/10 eventually delete this shim.
- `crates/regseq-backends/src/linux/conventions.rs` contains
  `generated_file_banner` relocated from `c_ast.rs`. The one
  call site in `linux/emitter.rs` imports from the new location.
- `backends::c::include_guard_name` retained (Linux-neutral).
- Phase-7C subset AST (statements: Compound, Decl, Expr, If, While,
  Return, Goto, Labeled, Empty, Raw; expressions: IntLiteral,
  StringLiteral, Ident, Field, Index, Call, Unary, Binary, Cast,
  Assign, Ternary, Paren, Raw; declarators/types/initializers per
  Implementation Notes) is implemented with render methods.
- Deferred constructs (`for`, `do while`, `switch`, `break`,
  `continue`, `typedef`, `enum`, `union`, preprocessor
  conditionals) are absent; their absence is documented in the
  module's top-of-file comment with a pointer to migration.md Q8.
- Unit tests in `c_ast_nodes.rs` pass.
- `cargo test --workspace` green (behavior unchanged).
- Codex review APPROVED.
- `git diff --name-only main..HEAD` scoped to
  `crates/regseq-backends/src/c/**`,
  `crates/regseq-backends/src/linux/{c_ast.rs,conventions.rs,emitter.rs}`,
  `crates/regseq-backends/src/lib.rs`,
  `crates/regseq-backends/tests/c_ast_nodes.rs`,
  plus `Cargo.lock` if applicable.
