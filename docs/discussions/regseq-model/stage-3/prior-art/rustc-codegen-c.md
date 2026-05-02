# rustc_codegen_c -- Stage-3 prior-art notes

Primary-source research for the Stage-3 type-system spec of the RegseqModel
compiler. The subject is `rustc-lang/rustc_codegen_c`, an experimental C
backend for rustc that lowers MIR through `rustc_codegen_ssa`'s trait
interface and prints C source.

## Repository confirmation

- Canonical repo: https://github.com/rust-lang/rustc_codegen_c (description:
  "C based backend for rustc"). The org is `rust-lang/`, not the standalone
  `rustc-codegen-c/rustc-codegen-c` mentioned in the prompt.
- Latest commit on `main` at time of research: `6044b30`, 2025-02-26,
  "feat: add basic support for function call (#12)" (verified via
  `https://api.github.com/repos/rust-lang/rustc_codegen_c/commits`).
- Pinned toolchain: `nightly-2024-07-01` with `rust-src`, `rustc-dev`,
  `llvm-tools-preview` (https://raw.githubusercontent.com/rust-lang/rustc_codegen_c/main/rust-toolchain.toml,
  lines 1-3).
- README explicitly tags the project as "highly experimental and not ready
  for production use." Only Linux is supported and `clang` is required
  (https://raw.githubusercontent.com/rust-lang/rustc_codegen_c/main/README.md,
  lines 7-24).
- Workspace is two crates:
  - `crates/rustc_codegen_c_ast/` -- the C AST itself (no rustc deps in
    `Cargo.toml`, but `rustc_private = true`).
  - `crates/rustc_codegen_c/` -- the rustc-side glue, depending on
    `rustc_codegen_c_ast` and many `rustc_*` private crates
    (https://raw.githubusercontent.com/rust-lang/rustc_codegen_c/main/crates/rustc_codegen_c/Cargo.toml,
    lines 1-15).

NOTE: Maturity matters for everything that follows. The C AST is the
mature artifact. The rustc-side bridge (`crates/rustc_codegen_c/src/`) is
mostly stubbed with `todo!()` -- only function predefine and "return a
constant / call a function with int args" actually work. Treat any
finding about lowering as "the start of a design", not "a finished
design".

---

## A. Type-vocabulary completeness

The type AST lives in
`crates/rustc_codegen_c_ast/src/ty.rs` (372 lines).

**Top-level enum (verbatim, lines 18-34):**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CTy<'mx> {
    /// The C `void` type.
    Void,
    /// The C boolean type.
    Bool,
    /// The C `char` type.
    Char,
    /// A signed integer type.
    Int(CIntTy),
    /// An unsigned integer type.
    UInt(CUintTy),
    /// A non-primitive C type, e.g. a pointer type.
    ///
    /// This is an interned reference to a complex type.
    Ref(Interned<'mx, CTyKind<'mx>>),
}
```

**Complex-type enum (verbatim, lines 177-183):**

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CTyKind<'mx> {
    /// A pointer type.
    Pointer(CTy<'mx>),
    /// An array type with element type and size.
    Array(CTy<'mx>, usize),
}
```

**Integer sub-vocabularies (verbatim, lines 96-148):**

```rust
pub enum CIntTy {
    Isize,
    I8,
    I16,
    I32,
    I64,
}
```

```rust
pub enum CUintTy {
    Usize,
    U8,
    U16,
    U32,
    U64,
}
```

So the entire C type vocabulary is:

- `void`, `_Bool`, `char`
- `i8`/`i16`/`i32`/`i64`/`isize` (signed)
- `u8`/`u16`/`u32`/`u64`/`usize` (unsigned)
- `Pointer(CTy)`
- `Array(CTy, usize)`

What is conspicuously **absent**:

- No struct / union / enum / record type.
- No function type. Functions live in a separate AST node
  (`CFuncKind` in `crates/rustc_codegen_c_ast/src/func.rs`,
  lines 17-29) that holds a return `CTy` and a `Vec<(CTy, CValue)>` of
  parameters; there is no first-class function-pointer `CTy` variant.
- No floating-point type (no `float`/`double` constructors).
- No `i128`/`u128` -- the conversion helpers explicitly
  `unimplemented!("i128 not supported yet")` /
  `unimplemented!("u128 not supported yet")` (ty.rs lines 194, 206).
- No `const` / `volatile` / `restrict` qualifiers anywhere on `CTy`
  (verified by reading the full file).
- No typedef / named-type variant.
- No vector / SIMD type.
- No vararg marker.

The mapping from rustc's `IntTy`/`UintTy` to `CTy` lives on `ModuleCtx`,
not on `CTy` itself (ty.rs lines 185-209). Rust `i32` becomes
`CTy::Int(CIntTy::I32)` whose `to_str()` returns `"int32_t"`, not C
`int` -- types are spelled with `<stdint.h>` exact-width typedefs
(lines 117-126). `isize`/`usize` both map to `size_t` (`SIZE_MAX`).

NOTE: In `CIntTy::to_str` (line 120) `Isize` is spelled `"size_t"`,
which is wrong on most platforms (`size_t` is unsigned). This is a
visible bug in the published code, not a deliberate decision.

---

## B. Layout / representation

Layout is **not stored on `CTy`**. The C AST has zero size/alignment/repr
information.

The bridge holds layout indirectly through rustc:

- `CodegenCx` implements `LayoutOfHelpers` and `FnAbiOfHelpers`
  (https://raw.githubusercontent.com/rust-lang/rustc_codegen_c/main/crates/rustc_codegen_c/src/context.rs,
  lines 84-103) -- so when the compiler asks for a layout, it goes back
  through `tcx.layout_of`, not through any cached field on `CTy`.
- `HasDataLayout::data_layout` is `todo!()` (context.rs line 80) and
  `HasTargetSpec::target_spec` is `todo!()` (context.rs line 62), so the
  bridge does not carry through-target data layout in any usable form
  yet.
- `LayoutTypeMethods::backend_type` is `todo!()` (layout_type.rs line
  13). Only `immediate_backend_type` works, and only for `TyKind::Int`
  and `TyKind::Uint`; everything else `todo!()`s
  (https://raw.githubusercontent.com/rust-lang/rustc_codegen_c/main/crates/rustc_codegen_c/src/context/layout_type.rs,
  lines 32-38).
- `is_backend_immediate` switches on `layout.abi`:
  `Scalar`/`Vector` are immediate; `ScalarPair`/`Uninhabited`/`Aggregate`
  are not (layout_type.rs lines 40-45). This is the full extent of
  representation logic in the bridge today.
- `fn_decl_backend_type` returns `CTy::Void` unconditionally, which
  appears to be a placeholder (layout_type.rs line 22).

Repr handling: there is **no** acknowledgment of `#[repr(C)]`,
`#[repr(transparent)]`, `#[repr(packed)]`, or `#[repr(...)]` discriminant
sizes anywhere in the codebase. Because aggregate lowering is not
implemented, the question of how to honour `repr` has not been answered
at all.

---

## C. Identity, interning, canonical form

The crate uses two-tier interning:

- `CTy::Ref` wraps `Interned<'mx, CTyKind<'mx>>` (ty.rs line 33). Pointer
  and array `CTyKind` values flow through the arena and are compared via
  pointer identity.
- The arena is declared in
  `crates/rustc_codegen_c_ast/src/arena.rs` (lines 9-15):

```rust
rustc_arena::declare_arena!([
    [] decl: CDeclKind<'tcx>,
    [] expr: CExprKind<'tcx>,
    [] func: CFuncKind<'tcx>,
    [] stmt: CStmtKind<'tcx>,
    [] ty: CTyKind<'tcx>,
]);
```

- All AST nodes (decls, exprs, funcs, stmts, types) live in a single
  per-`ModuleCtx` arena (`crates/rustc_codegen_c_ast/src/lib.rs` lines
  29-69). Identity is by-arena, scoped to one C file/module.
- `CTy` itself is `Copy` (ty.rs line 18), and primitives don't need
  interning (they're plain enum variants); only the recursive
  `CTyKind` payload is interned.
- However, the actual interning *creation* path is missing. There is
  no `intern_ty`/`mk_pointer`/`mk_array` constructor anywhere in the
  source tree. The only places `CTy::Ref(Interned::new_unchecked(...))`
  appear are inside the unit tests in `ty.rs` (lines 307-368). So while
  `CTyKind` is shaped for interning, the codegen path doesn't actually
  build aggregate types yet.

Form: structural for primitives (the enum variants are the canonical
form), nominal-via-pointer-identity for the two complex kinds. There is
no separate "structural equivalence after monomorphization" pass;
arena identity is the canonical form.

`CFunc` is also `Interned`:

```rust
pub type CFunc<'mx> = Interned<'mx, CFuncKind<'mx>>;
```

(`crates/rustc_codegen_c_ast/src/func.rs` line 14). `CDecl`, `CExpr`,
`CStmt` are plain `&'mx CXxxKind<'mx>` arena references (decl.rs line
9, expr.rs line 22, stmt.rs line 9) -- intended for arena uniqueness
without `Interned`'s pointer-equality-by-arena guarantees.

---

## D. Special-semantics markers

Not applicable: the project records **no** Rust-specific semantic
markers at the C-type level. There is no `UnsafeCell`, `MaybeUninit`,
`PhantomData`, atomic, volatile, or aliasing flag on `CTy`. There is
no `const` qualifier either.

The only "Rust-aware" piece of the C AST is the `__rust_utos` macro
in the helper header
(https://raw.githubusercontent.com/rust-lang/rustc_codegen_c/main/crates/rustc_codegen_c/src/helper.h,
lines 9-12). That macro implements the unsigned-to-signed integer cast
that "avoids UB when do integer casting in Rust" -- a *semantic* fix
expressed at expression-codegen time, not at type-encoding time. The
helper is included verbatim into every emitted file via
`compile_codegen_unit` (`crates/rustc_codegen_c/src/base.rs` line 13:
`const HELPER: &str = include_str!("./helper.h");`).

So the design pattern here is "preserve Rust semantics in emitted
expressions, not in the type encoding."

---

## E. Post-monomorphization erasure

The project does consume generic Rust, but the bridge runs through
`rustc_codegen_ssa`'s standard `compile_codegen_unit` pipeline:

- `crates/rustc_codegen_c/src/base.rs` lines 42-61: `module_codegen`
  iterates `cgu.items_in_deterministic_order(tcx)` and calls
  `mono_item.predefine` then `mono_item.define` for each. By the time
  `predefine` runs, items are `Instance<'tcx>` -- monomorphized.
- `predefine_fn`
  (https://raw.githubusercontent.com/rust-lang/rustc_codegen_c/main/crates/rustc_codegen_c/src/context/pre_define.rs,
  lines 22-37) calls `self.fn_abi_of_instance(instance, ty::List::empty())`,
  meaning generics have already been substituted; the FnAbi the bridge
  walks is the post-substitution one. Each `arg.layout` and the return
  layout flow into `immediate_backend_type`.
- Lifetimes are erased before this point by rustc's normal pipeline
  (the bridge does not see them; `ParamEnv::reveal_all()` is used in
  `context.rs` line 68).

What survives into the C AST: only the resolved scalar widths and
signedness of arguments and returns (because that is all
`immediate_backend_type` handles before `todo!()`).

What is erased and gone: trait info, lifetimes, generic parameters,
`PhantomData`, `dyn Trait` -- everything beyond integer scalars is
unreachable in the current implementation.

Vtable and dyn handling: stubbed. `MiscMethods::vtables` returns
`todo!()` (https://raw.githubusercontent.com/rust-lang/rustc_codegen_c/main/crates/rustc_codegen_c/src/context/misc.rs,
lines 12-17). No design exists yet.

---

## F. Bridge / consumption shape

The bridge is `crates/rustc_codegen_c/src/`. The shape:

- `CCodegen` implements `CodegenBackend`, `ExtraBackendMethods`, and
  `WriteBackendMethods` (lib.rs lines 56-241). Most LTO and target
  hooks are `unimplemented!()` (lines 138, 174, 186, 191, 194, 211,
  217, 233, 239) or no-ops.
- `CodegenCx` (`crates/rustc_codegen_c/src/context.rs` lines 33-47)
  associates `tcx`, `ModuleCtx`, and a function-instance map:

```rust
pub struct CodegenCx<'tcx, 'mx> {
    pub tcx: TyCtxt<'tcx>,
    pub mcx: ModuleCtx<'mx>,
    pub function_instances: RefCell<FxHashMap<Instance<'tcx>, CFunc<'mx>>>,
}
```

- `CodegenCx::new` immediately pushes `<stdint.h>` as an include
  (context.rs line 44). Every emitted C file gets `<stdint.h>` plus
  the helper header.
- The `BackendTypes` impl pegs the SSA backend's associated types
  (context.rs lines 49-58):

```rust
type Value = CValue<'mx>;
type Function = CFunc<'mx>;
type BasicBlock = CFunc<'mx>;
type Type = CTy<'mx>;
type Funclet = ();
type DIScope = ();
type DILocation = ();
type DIVariable = ();
```

Notable: `BasicBlock` is identified with `Function`. The C printer has
no concept of basic blocks -- the function body is a flat
`Vec<CStmt<'mx>>`. The bridge therefore can't lower MIR's CFG faithfully;
it has to flatten or sequence.

- The "C-shape" lowering decisions baked into the AST today:
  - Functions are not first-class types; they're a separate AST node
    (`CFuncKind`) printed via `print_func_decl` /
    `print_signature` (`crates/rustc_codegen_c_ast/src/func.rs`
    lines 74-91). No function-pointer `CTy` variant exists.
  - Local variables are numbered (`CValue::Local(usize)`, expr.rs lines
    11-19), echoing MIR's `_0`, `_1`, etc. The blessed C output
    confirms (`tests/bless/basic_math.c`): parameters are `_0`, `_1`,
    `_2`; the return temp is `_3`.
  - Function-name resolution at call sites is by *string match on
    the last `::` segment of the def-path*
    (`crates/rustc_codegen_c/src/context/misc.rs` lines 23-29):

```rust
fn get_fn_addr(&self, instance: Instance<'tcx>) -> Self::Value {
    let funcs = self.mcx.module().funcs.borrow();
    let path = self.tcx.def_path_debug_str(instance.def_id());
    let name = path.split("::").last().unwrap();
    let func = funcs.iter().find(|f| f.0.name == name).unwrap();
    CValue::Func(func.0.name)
}
```

NOTE: This is a live anti-pattern. It works only when last-segment
names are unique within a CGU and panics with "unwrap" if not.

- Output type from `compile_codegen_unit` is `String` (lib.rs line
  155: `type Module = String;`). `module_codegen` converts the
  `ModuleCtx` to a `String` via its `Display` impl (base.rs line 59;
  `crates/rustc_codegen_c_ast/src/lib.rs` lines 50-56) and returns it
  inside a `ModuleCodegen<String>`. The downstream "compile" step
  is just a write of that string and a clang invocation handled in
  `crates/rustc_codegen_c/src/write.rs`.

- Examples confirm the pipeline produces text exactly matching the
  blessed `.c` (https://raw.githubusercontent.com/rust-lang/rustc_codegen_c/main/tests/bless/basic_math.c)
  for the trivial integer-only programs in `examples/basic_math.rs`.

---

## G. Renderer / pretty-printer format

The pretty-printer is built on rustc's own pretty-printing infrastructure
(`crates/rustc_codegen_c_ast/src/pretty.rs`, lines 1-149). It wraps
`rustc_ast_pretty::pp::Printer` with helpers for boxes (consistent /
inconsistent / visual-aligned), delimiters, soft / hard / zero breaks,
and a separator iterator.

The interesting design decision is in
`crates/rustc_codegen_c_ast/src/ty.rs` lines 211-276: rendering goes
through a dedicated `print_declarator` function that handles C's
"declarator" syntax explicitly. Doc comment (lines 211-220):

```
/// Print a C declarator.
///
/// A declarator is a type with an optional identifier and pointer indirections,
/// e.g. `int *x`.
///
/// This function is necessary because the C declarator syntax is quite complex
/// when the type becomes more complex, e.g. `int (*x)[10]`.
///
/// When `val` is `None`, this prints an abstract declarator, or in other words,
/// a standalone type without an identifier.
```

The algorithm walks down through `CTyKind::Pointer` and `CTyKind::Array`,
pushing declarator parts onto a `VecDeque` (front for pointers and
opening parens, back for array dimensions and closing parens). Pointer-
to-array correctly produces parenthesised forms (`int32_t (*x)[10]`) --
the unit tests in lines 348-371 verify both `int32_t (*_3)[10]` and
`int32_t (*_4[3])[5]`.

`Print` trait (pretty.rs line 147):

```rust
pub trait Print {
    fn print_to(&self, ctx: &mut PrinterCtx);
}
```

`Module::print_to` (`crates/rustc_codegen_c_ast/src/module.rs` lines
49-83) emits in a fixed order:

1. `#include <...>` lines.
2. The helper-header text inlined verbatim.
3. All declarations.
4. Forward declarations of all functions (signatures + `;`).
5. Bodies of all functions.

This matches the blessed output exactly.

`CFunc::print_to` (func.rs lines 64-91) calls `print_signature` then
`print_compound` for the body. `print_signature` uses
`print_declarator` with the function name (`CValue::Func`) so the
function name and return type share the same declarator machinery as
variables.

Worth noting:

- `CValue::Local(i)` is rendered as `_{i}` (expr.rs line 87) -- the
  classic MIR-style local-name convention bleeds into the C output.
- The pretty-printer module does not insert any C99 / C11 specific
  qualifiers (`const`, `restrict`); it only knows how to interleave
  `*`, `[N]`, `(`, `)`, identifier, primitive name.

---

## Watch-outs and observations

1. **The C AST is much simpler than what's needed for general Rust.**
   Five primitive kinds plus `Pointer` and `Array` is the entire
   vocabulary. There is no struct, union, enum, function-pointer type,
   floating-point type, typedef, or qualifier representation. RegseqModel
   needs to add at least: aggregates, function-pointer, float, and a
   way to attach `repr`/layout decisions -- the rustc_codegen_c crate
   has done none of that work yet. Treat the absence as "open question",
   not as "considered and rejected."

2. **Layout lives outside the type, not in it.** Layout is queried
   through `tcx.layout_of` on demand from the SSA helper traits
   (`LayoutOfHelpers`/`FnAbiOfHelpers` on `CodegenCx`). `CTy` carries
   no `layout`/`size`/`align` field. Whether to follow this pattern
   for RegseqModel (lazy-from-tcx) or store resolved layout on the
   type node is an explicit choice the project has not yet had to face.

3. **Two-tier interning with primitives outside the arena.** Primitives
   are plain enum variants; only `Pointer`/`Array` (the recursive
   kinds) go through `Interned`. This makes `CTy: Copy` cheap -- it's
   a `(tag, payload)` of at most pointer size. Worth borrowing.

4. **No type-naming / typedef discipline.** The C output never emits
   `typedef`s, never uses Rust struct names, and identifies functions
   by C symbol name. This means name-mangling decisions, ABI-symbol
   resolution, and "what to put in the header for a Rust enum" are
   *not* C-AST concerns in this project. They will be in Regseq.

5. **Live anti-pattern: function-name resolution by string-split-on-`::`.**
   `MiscMethods::get_fn_addr` does
   `tcx.def_path_debug_str(...).split("::").last().unwrap()`
   (`context/misc.rs` lines 23-29). It will silently collide on
   homonym functions in different modules and panics if the function
   isn't in the CGU. Do not copy this. Use the resolved `Instance`
   directly to look up emitted symbols.

6. **`<stdint.h>` exact-width spelling, not C's native `int`/`long`.**
   Every integer is rendered through the `intN_t`/`uintN_t` family,
   matching what rustc tells us about the bit width. The mapping is
   pure: `i32 -> int32_t`, `u8 -> uint8_t`, etc. (`ty.rs` lines 117-126,
   151-160). This sidesteps "C `int` is whatever the compiler says it
   is". Recommended pattern for RegseqModel.

7. **`isize` -> `size_t` is wrong (signedness).** `CIntTy::Isize` maps
   to `"size_t"` and `"SIZE_MAX"` (`ty.rs` lines 120, 131). `size_t`
   is unsigned. The intent was probably `intptr_t`/`PTRDIFF_MAX` or
   `ssize_t`. This is a published bug in a paper-backed project;
   double-check the `isize`/`usize` mapping in any RegseqModel
   bridge.

8. **`i128`/`u128` are explicit `unimplemented!()` panics, not
   silent fallbacks.** ty.rs lines 194 and 206. Consistent with the
   project's experimental status; aligns with our "no defensive
   fallbacks" rule -- the right behaviour is to hard-error on
   unsupported widths.

9. **Special-semantics markers (`UnsafeCell`, `MaybeUninit`,
   `PhantomData`, atomics, volatile) are entirely absent from the
   type representation.** No `volatile` qualifier on pointer types,
   no atomic fence emission. For a device-driver compiler this is the
   single largest gap in the prior art -- RegseqModel cannot adopt
   `rustc_codegen_c`'s type vocabulary as-is for MMIO work.

10. **Functions are not types.** `CFuncKind` is a separate AST node;
    `CTy` has no `Function`/`FunctionPointer` variant. Function values
    are represented as `CValue::Func(&'mx str)` (a string symbol), not
    as a typed pointer. Calling-convention info is **not** carried on
    the function representation either. For RegseqModel this needs to
    be reconsidered (extern signatures, callbacks, vtable slots all
    need typed function pointers).

11. **`BasicBlock` is identified with `Function`** in the
    `BackendTypes` impl (`context.rs` line 52: `type BasicBlock = CFunc<'mx>;`).
    The C AST has no MIR-style basic-block notion. The body is a
    `Vec<CStmt>`. This is consistent with C semantics (no goto-table
    structure required) but means MIR CFG must be lowered to
    structured C control flow -- and that lowering hasn't been
    implemented yet (no `if`/`while`/`switch`/`goto` in `CStmtKind`,
    only `Compound`, `Return`, `Decl`, `Expr` --
    `crates/rustc_codegen_c_ast/src/stmt.rs` lines 12-22). Another
    big "design unfinished" mark.

12. **Pretty-printer reuses rustc's `rustc_ast_pretty::pp` Printer.**
    `crates/rustc_codegen_c_ast/src/pretty.rs` line 6:
    `use rustc_ast_pretty::pp;`. Boxes / breaks / soft-vs-hard line
    breaks are exactly the same machinery rustc uses for Rust source.
    RegseqModel can do the same (we already depend on `rustc_private`)
    or roll its own; either is precedented.

13. **The declarator-based renderer is a clean, testable pattern.**
    `print_declarator` walks `CTyKind` while pushing parts onto a
    `VecDeque`, resulting in correct C "spiral" syntax for pointers
    to arrays, arrays of pointers, etc. The unit tests in `ty.rs`
    lines 288-371 are a good template: each test builds a `CTy`,
    runs `print_declarator`, asserts on a string. Recommended pattern.

14. **No `repr` / `#[repr(C)]` handling exists.** Because aggregates
    aren't in the AST, the project has not had to confront how to
    honour Rust's `repr` attributes when emitting C structs. This is
    a wide-open gap in the prior art.

15. **Maturity asymmetry.** The C AST crate (`rustc_codegen_c_ast`) is
    a coherent, internally consistent, tested subset of C. The bridge
    crate (`rustc_codegen_c`) is mostly `todo!()` placeholders --
    `BaseTypeMethods` is fully `todo!()` (every method,
    `crates/rustc_codegen_c/src/context/base_type.rs` lines 1-85);
    `ConstMethods` is essentially fully `todo!()` except an integer
    `Scalar::Int` path (`crates/rustc_codegen_c/src/context/const.rs`
    lines 7-111); `BuilderMethods` is dozens of `todo!()` lines
    (`crates/rustc_codegen_c/src/builder.rs` shows ~50 `todo!()`
    around lines 48-330). The "design lessons" we can extract are
    primarily from the AST crate, not the bridge.

16. **Repo activity.** Latest commit 2025-02-26. No commits in 14
    months as of 2026-04-28. The project is best treated as a
    snapshot of an early design, not a moving target.
