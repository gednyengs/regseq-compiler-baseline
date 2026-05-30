#![no_std]
#![feature(register_tool)]
#![feature(core_intrinsics)]
#![register_tool(rseq)]

// B3b — references / raw pointers / slices / str / fat-pointer rvalues + the
// pointer-flavoured intrinsics (PtrMetadata, Unsize, Offset, Assume,
// CopyNonOverlapping). Each info! line computes a value THROUGH the construct
// and logs it. POINTERS are never logged as raw addresses (non-deterministic);
// they are observed via boolean relations or by dereferencing the pointee. The
// `// proves:` comment maps each line to the Doc-1 matrix row(s) it drives.

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                U8(u8), U16(u16), U32(u32), U64(u64),
                I8(i8), I16(i16), I32(i32), I64(i64),
                Bool(bool), Str(&'a str),
            }
            impl From<u8> for LogArg<'_> { fn from(v: u8) -> Self { LogArg::U8(v) } }
            impl From<u16> for LogArg<'_> { fn from(v: u16) -> Self { LogArg::U16(v) } }
            impl From<u32> for LogArg<'_> { fn from(v: u32) -> Self { LogArg::U32(v) } }
            impl From<u64> for LogArg<'_> { fn from(v: u64) -> Self { LogArg::U64(v) } }
            impl From<i8> for LogArg<'_> { fn from(v: i8) -> Self { LogArg::I8(v) } }
            impl From<i16> for LogArg<'_> { fn from(v: i16) -> Self { LogArg::I16(v) } }
            impl From<i32> for LogArg<'_> { fn from(v: i32) -> Self { LogArg::I32(v) } }
            impl From<i64> for LogArg<'_> { fn from(v: i64) -> Self { LogArg::I64(v) } }
            impl From<bool> for LogArg<'_> { fn from(v: bool) -> Self { LogArg::Bool(v) } }
            impl<'a> From<&'a str> for LogArg<'a> { fn from(v: &'a str) -> Self { LogArg::Str(v) } }

            #[inline(never)] #[rseq::intrinsic("log-debug")]
            pub fn log_debug(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-warn")]
            pub fn log_warn(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-error")]
            pub fn log_error(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! debug { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_debug($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use debug;
        macro_rules! info { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_info($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use info;
        macro_rules! log_warn { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_warn($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use log_warn as warn;
        macro_rules! error { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_error($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use error;
    }
}

// A static referenced by-address from regseq_main → CO-11 (Const::Reloc:
// address-of an instance) and CO-14 (Byte::Reloc: a static whose initializer is
// a pointer to another static, i.e. a relocation in the byte image).
static TARGET: u32 = 0xABCD;          // proves: CO-11 target
// CO-14 (Byte::Reloc): a NON-scalar static (array of references) whose byte
// image embeds a relocation. A THIN `&u32` static would take the scalar path
// (Const::Reloc, not Byte::Reloc); an aggregate static `[&u32; 1]` goes through
// allocation_bytes (const_.rs:307), so the reference's address bytes become a
// genuine Byte::Reloc in a Const::Bytes image.
static REF_ARR: [&u32; 1] = [&TARGET]; // proves: CO-14 (Byte::Reloc in a Const::Bytes image)

// A union static whose byte image has an unwritten 4th byte (the u32 view is 4
// bytes; only the 3-byte array view is initialized). NOTE: this does NOT prove
// CO-12 (Byte::Uninit) — the backend emits the unwritten byte as an
// indistinguishable 0, so a green run cannot tell genuine uninit handling from
// Byte::Value(0) (Codex soundness pass). CO-12 is deferred until a golden-level
// oracle exposes uninit distinctly. The union read below still exercises the
// union-place + byte-image const path for the rows that ARE claimed.
union UPad { w: u32, b3: [u8; 3] }
static UPAD: UPad = UPad { b3: [0x77, 0x88, 0x99] };

// A `const` null pointer: const-eval folds `0 as *const i32` to a scalar-0 with
// pointer type, which the bridge translates to Const::NullPtr (const_.rs:509) —
// the genuine CO-10 surface (a runtime cast would be a Cast rvalue instead).
const NULLP: *const i32 = 0 as *const i32; // proves: CO-10 (Const::NullPtr)

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- reference rvalue `&x` (RV-3 / TY-10) + deref place `*p` (PL-2) ---
    let x: i32 = 314;
    let r: &i32 = &x;                                 // proves: RV-3 (Rvalue::Ref), TY-10 (Ref)
    regseq::log::info!("deref_r={}", *r);            // 314  proves: PL-2 (Deref)

    // --- raw pointer rvalue `&raw const x` (RV-4 / TY-11) ---
    let p: *const i32 = &raw const x;                // proves: RV-4 (Rvalue::RawPtr), TY-11 (Ptr)
    // observe the pointer relationally (never a raw address): it is non-null,
    // and dereferences back to x.
    regseq::log::info!("p_nonnull={}", (p as usize != 0) as u8);     // 1
    regseq::log::info!("p_deref_eq={}", (unsafe { *p } == x) as u8); // 1  proves: PL-2 via raw ptr

    // --- array → slice unsize coercion `&[u8;N] -> &[u8]` (PC-4 / TY-9) ---
    let arr: [u8; 5] = [10, 20, 30, 40, 50];
    let s: &[u8] = &arr;                             // proves: PC-4 (Unsize), TY-9 (Slice)
    regseq::log::info!("s0={}", s[0]);              // 10  (slice index)
    regseq::log::info!("s_last={}", s[4]);          // 50

    // --- slice metadata = length, WITHOUT .len() — via ptr_metadata intrinsic.
    // ptr_metadata lowers to Rvalue::UnaryOp(PtrMetadata) and for *const [u8]
    // returns the usize length (UN-3). ---
    let smeta: usize = core::intrinsics::ptr_metadata(s as *const [u8]); // proves: UN-3 (PtrMetadata)
    regseq::log::info!("s_len={}", smeta as u64);   // 5

    // --- str behind a reference (TY-6): &str is a fat pointer; we observe its
    // metadata (byte length) via ptr_metadata, and a byte through the data ptr. ---
    // (the first-byte read via `st as *const str as *const u8` is omitted: the
    // backend cannot cast a fat `*const str` to a thin `*const u8` — "cannot
    // convert to a pointer type". TY-6 is fully driven by the metadata read,
    // which proves the str is a fat pointer behind a Ref.) ---
    let st: &str = "hello";                          // proves: TY-6 (Str behind Ref)
    let stlen: usize = core::intrinsics::ptr_metadata(st as *const str); // len = 5
    regseq::log::info!("st_len={}", stlen as u64);  // 5

    // --- fat raw-ptr aggregate `*const [u8]` (AG-7): build a fat raw pointer
    // from a thin data pointer + length metadata. A `s as *const [u8]` cast only
    // COPIES the existing fat pointer (no aggregate built); and
    // `core::ptr::slice_from_raw_parts` is a library fn (no inline → undefined
    // ref). The genuine AG-7 surface is the `aggregate_raw_ptr` intrinsic, which
    // lowers to Rvalue::Aggregate(AggregateKind::RawPtr, [data, meta]). We build
    // a `*const [u8]` of length 3 over the data pointer of arr. Observe via its
    // metadata (3) + the deref'd first element (10). ---
    let datap: *const u8 = &raw const arr as *const u8;
    let fat: *const [u8] =
        core::intrinsics::aggregate_raw_ptr::<*const [u8], *const u8, usize>(datap, 3); // proves: AG-7
    let fatlen: usize = core::intrinsics::ptr_metadata(fat);
    regseq::log::info!("fat_len={}", fatlen as u64); // 3  (the metadata of the built fat ptr)

    // --- PL-5 / PL-6 / PL-7 slice places via a slice pattern. Matching
    // `[first, .., last]` binds `first` = s[0] (ConstantIndex from_end:false,
    // PL-5) and `last` = s[len-1] (ConstantIndex from_end:true, PL-6). A
    // `[head, tail @ ..]` binding produces a Subslice place (PL-7). ---
    if let [first, .., last] = *s {                  // proves: PL-6 (ConstantIndex from_end:true)
        regseq::log::info!("first={} last={}", first, last); // 10 50  (first = PL-5/PL-1, last = PL-6)
    }
    if let [_head, tail @ ..] = s {                  // proves: PL-7 (Subslice)
        // tail is &[u8] = s[1..]; observe its length (4) + first element (20).
        let tlen: usize = core::intrinsics::ptr_metadata(tail as *const [u8]);
        regseq::log::info!("tail_len={} tail0={}", tlen as u64, tail[0]); // 4 20
    }

    // --- BO-26 (Offset): pointer arithmetic without .offset()/.add() — via the
    // offset intrinsic (lowers to Rvalue::BinaryOp(Offset)). p2 = &arr[0] + 2
    // points at arr[2] = 30. ---
    let base: *const u8 = &raw const arr as *const u8;
    let p2: *const u8 = unsafe { core::intrinsics::offset(base, 2isize) }; // proves: BO-26 (Offset)
    regseq::log::info!("offset2={}", unsafe { *p2 }); // 30

    // --- ST-3 (Assume): assume a runtime-true predicate; observe the guarded
    // computation still runs and is correct. ---
    let n: u8 = arr[1];                              // 20
    unsafe { core::intrinsics::assume(n > 0) };      // proves: ST-3 (Assume)
    regseq::log::info!("assume_ok={}", n);           // 20

    // --- ST-4 (CopyNonOverlapping): memcpy 3 u8 from src into dst, observe. ---
    let src: [u8; 3] = [7, 8, 9];
    let mut dst: [u8; 3] = [0, 0, 0];
    unsafe {
        core::intrinsics::copy_nonoverlapping(
            &raw const src as *const u8,
            &raw mut dst as *mut u8,
            3,
        );                                           // proves: ST-4 (CopyNonOverlapping)
    }
    regseq::log::info!("copied={} {} {}", dst[0], dst[1], dst[2]); // 7 8 9

    // --- CO-10 (NullPtr): a `const` null pointer. A runtime `0usize as *const`
    // is a Cast rvalue (NOT Const::NullPtr). Const::NullPtr arises when a
    // CTFE-evaluated scalar-0 is translated with pointer type (const_.rs:509). A
    // `const` item of `*const i32` whose value is null const-evaluates to exactly
    // that. Observe relationally (is-null), never as an address. ---
    let np: *const i32 = NULLP;                      // proves: CO-10 (Const::NullPtr)
    regseq::log::info!("is_null={}", (np as usize == 0) as u8); // 1

    // --- CO-11 (Reloc): reference the static TARGET by address. A thin `&TARGET`
    // is the scalar reference path → a Const::Reloc carrier (address-of an
    // instance). Read the relocated value and confirm the address is non-null. ---
    let tref: &u32 = &TARGET;                        // proves: CO-11 (Const::Reloc: address-of static)
    regseq::log::info!("target={}", *tref);          // 43981 (0xABCD)
    regseq::log::info!("target_nonnull={}", (tref as *const u32 as usize != 0) as u8); // 1

    // --- CO-14 (Byte::Reloc): REF_ARR is a `[&u32; 1]` static — a NON-scalar
    // static, so its initializer goes through allocation_bytes and the embedded
    // reference's address bytes become a genuine Byte::Reloc in a Const::Bytes
    // image. Dereference the single element back to TARGET's value to confirm. ---
    regseq::log::info!("ref_reloc={}", *REF_ARR[0]); // 43981  proves: CO-14 (Byte::Reloc)

    // --- CO-13 (Byte::Value): every `info!` format-string `&str` literal is a
    // Const::Bytes whose bytes are Byte::Value (the string's bytes); they are
    // emitted as const_memory_static char arrays. Abundantly driven by every
    // logged line above — observed structurally (the strings print correctly). ---
    regseq::log::info!("co13_marker={}", 1u8);       // 1   proves: CO-13 (Byte::Value &str bytes)

    // --- union-static byte-image read (the unwritten 4th byte is NOT a CO-12
    // proof — see the UPAD definition note; CO-12 deferred). ---
    let u0 = unsafe { UPAD.b3[0] };
    let u1 = unsafe { UPAD.b3[1] };
    let u2 = unsafe { UPAD.b3[2] };
    regseq::log::info!("upad={} {} {}", u0, u1, u2); // 119 136 153
}
