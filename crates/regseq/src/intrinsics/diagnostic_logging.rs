/// Typed argument carrier for the canonical logging anchors.
///
/// `LogArg` encodes the accepted primitive argument set for Regseq logging.
/// The compiler lowers these to backend-native format specifiers at compile
/// time. `core::fmt::Arguments` and `Display`/`Debug` are not part of the
/// stable contract.
#[derive(Debug, Clone, Copy)]
pub enum LogArg<'a> {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    Usize(usize),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    Isize(isize),
    Bool(bool),
    Str(&'a str),
    Ptr(*const ()),
}

impl From<u8> for LogArg<'_> {
    fn from(v: u8) -> Self {
        LogArg::U8(v)
    }
}
impl From<u16> for LogArg<'_> {
    fn from(v: u16) -> Self {
        LogArg::U16(v)
    }
}
impl From<u32> for LogArg<'_> {
    fn from(v: u32) -> Self {
        LogArg::U32(v)
    }
}
impl From<u64> for LogArg<'_> {
    fn from(v: u64) -> Self {
        LogArg::U64(v)
    }
}
impl From<usize> for LogArg<'_> {
    fn from(v: usize) -> Self {
        LogArg::Usize(v)
    }
}
impl From<i8> for LogArg<'_> {
    fn from(v: i8) -> Self {
        LogArg::I8(v)
    }
}
impl From<i16> for LogArg<'_> {
    fn from(v: i16) -> Self {
        LogArg::I16(v)
    }
}
impl From<i32> for LogArg<'_> {
    fn from(v: i32) -> Self {
        LogArg::I32(v)
    }
}
impl From<i64> for LogArg<'_> {
    fn from(v: i64) -> Self {
        LogArg::I64(v)
    }
}
impl From<isize> for LogArg<'_> {
    fn from(v: isize) -> Self {
        LogArg::Isize(v)
    }
}
impl From<bool> for LogArg<'_> {
    fn from(v: bool) -> Self {
        LogArg::Bool(v)
    }
}
impl<'a> From<&'a str> for LogArg<'a> {
    fn from(v: &'a str) -> Self {
        LogArg::Str(v)
    }
}
impl From<*const ()> for LogArg<'_> {
    fn from(v: *const ()) -> Self {
        LogArg::Ptr(v)
    }
}

/// Canonical logging anchor: error level.
#[inline(never)]
#[rseq::intrinsic]
pub fn error<'a, const N: usize>(_template: &'static str, _args: [LogArg<'a>; N]) {
    unimplemented!("compiler-managed: diagnostic_logging::error")
}

/// Canonical logging anchor: warn level.
#[inline(never)]
#[rseq::intrinsic]
pub fn warn<'a, const N: usize>(_template: &'static str, _args: [LogArg<'a>; N]) {
    unimplemented!("compiler-managed: diagnostic_logging::warn")
}

/// Canonical logging anchor: info level.
#[inline(never)]
#[rseq::intrinsic]
pub fn info<'a, const N: usize>(_template: &'static str, _args: [LogArg<'a>; N]) {
    unimplemented!("compiler-managed: diagnostic_logging::info")
}

/// Canonical logging anchor: debug level.
#[inline(never)]
#[rseq::intrinsic]
pub fn debug<'a, const N: usize>(_template: &'static str, _args: [LogArg<'a>; N]) {
    unimplemented!("compiler-managed: diagnostic_logging::debug")
}
