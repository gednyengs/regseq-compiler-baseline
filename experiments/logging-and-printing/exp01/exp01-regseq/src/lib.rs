use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub trait IntoLogArg<'a> {
    fn into_log_arg(self) -> LogArg<'a>;
}

macro_rules! impl_into_log_arg {
    ($ty:ty, $variant:ident) => {
        impl<'a> IntoLogArg<'a> for $ty {
            fn into_log_arg(self) -> LogArg<'a> {
                LogArg::$variant(self)
            }
        }
    };
}

impl_into_log_arg!(u8, U8);
impl_into_log_arg!(u16, U16);
impl_into_log_arg!(u32, U32);
impl_into_log_arg!(u64, U64);
impl_into_log_arg!(usize, Usize);
impl_into_log_arg!(i8, I8);
impl_into_log_arg!(i16, I16);
impl_into_log_arg!(i32, I32);
impl_into_log_arg!(i64, I64);
impl_into_log_arg!(isize, Isize);
impl_into_log_arg!(bool, Bool);

impl<'a> IntoLogArg<'a> for &'a str {
    fn into_log_arg(self) -> LogArg<'a> {
        LogArg::Str(self)
    }
}

impl<'a, T> IntoLogArg<'a> for *const T {
    fn into_log_arg(self) -> LogArg<'a> {
        LogArg::Ptr(self.cast())
    }
}

impl<'a, T> IntoLogArg<'a> for *mut T {
    fn into_log_arg(self) -> LogArg<'a> {
        LogArg::Ptr(self.cast())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEvent<'a, const N: usize> {
    pub level: LogLevel,
    pub template: &'static str,
    pub args: [LogArg<'a>; N],
}

pub mod intrinsics {
    pub mod diagnostic_logging {
        use crate::{LogArg, LogEvent, LogLevel};

        pub fn error<'a, const N: usize>(
            template: &'static str,
            args: [LogArg<'a>; N],
        ) -> LogEvent<'a, N> {
            LogEvent {
                level: LogLevel::Error,
                template,
                args,
            }
        }

        pub fn warn<'a, const N: usize>(
            template: &'static str,
            args: [LogArg<'a>; N],
        ) -> LogEvent<'a, N> {
            LogEvent {
                level: LogLevel::Warn,
                template,
                args,
            }
        }

        pub fn info<'a, const N: usize>(
            template: &'static str,
            args: [LogArg<'a>; N],
        ) -> LogEvent<'a, N> {
            LogEvent {
                level: LogLevel::Info,
                template,
                args,
            }
        }

        pub fn debug<'a, const N: usize>(
            template: &'static str,
            args: [LogArg<'a>; N],
        ) -> LogEvent<'a, N> {
            LogEvent {
                level: LogLevel::Debug,
                template,
                args,
            }
        }
    }
}

#[macro_export]
macro_rules! __regseq_log_error {
    ($template:literal $(, $arg:expr)* $(,)?) => {
        $crate::intrinsics::diagnostic_logging::error(
            $template,
            [$($crate::IntoLogArg::into_log_arg($arg)),*],
        )
    };
}

#[macro_export]
macro_rules! __regseq_log_warn {
    ($template:literal $(, $arg:expr)* $(,)?) => {
        $crate::intrinsics::diagnostic_logging::warn(
            $template,
            [$($crate::IntoLogArg::into_log_arg($arg)),*],
        )
    };
}

#[macro_export]
macro_rules! __regseq_log_info {
    ($template:literal $(, $arg:expr)* $(,)?) => {
        $crate::intrinsics::diagnostic_logging::info(
            $template,
            [$($crate::IntoLogArg::into_log_arg($arg)),*],
        )
    };
}

#[macro_export]
macro_rules! __regseq_log_debug {
    ($template:literal $(, $arg:expr)* $(,)?) => {
        $crate::intrinsics::diagnostic_logging::debug(
            $template,
            [$($crate::IntoLogArg::into_log_arg($arg)),*],
        )
    };
}

pub mod log {
    pub use crate::__regseq_log_debug as debug;
    pub use crate::__regseq_log_error as error;
    pub use crate::__regseq_log_info as info;
    pub use crate::__regseq_log_warn as warn;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Linux,
    Zephyr,
    Windows,
    FreeBsd,
    MacOs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaceholderKind {
    Default,
    LowerHex,
    UpperHex,
    Pointer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlaceholderStyle {
    kind: PlaceholderKind,
    alternate: bool,
    zero_pad_width: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    PlaceholderCountMismatch { placeholders: usize, args: usize },
    UnsupportedSpecifier(String),
    InvalidPointerStyle,
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlaceholderCountMismatch { placeholders, args } => {
                write!(
                    f,
                    "placeholder count {} does not match argument count {}",
                    placeholders, args
                )
            }
            Self::UnsupportedSpecifier(spec) => write!(f, "unsupported specifier `{}`", spec),
            Self::InvalidPointerStyle => write!(f, "pointer arguments only support `{{:p}}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredFormat {
    pub backend: Backend,
    pub format_string: String,
}

impl<'a, const N: usize> LogEvent<'a, N> {
    pub fn lower_format(&self, backend: Backend) -> Result<LoweredFormat, LowerError> {
        let styles = parse_template(self.template)?;
        if styles.len() != N {
            return Err(LowerError::PlaceholderCountMismatch {
                placeholders: styles.len(),
                args: N,
            });
        }

        let mut rendered = String::new();
        let mut arg_idx = 0usize;
        let mut chars = self.template.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch != '{' {
                rendered.push(ch);
                continue;
            }

            let mut spec = String::new();
            for next in chars.by_ref() {
                if next == '}' {
                    break;
                }
                spec.push(next);
            }

            let style = parse_placeholder(&spec)?;
            rendered.push_str(&spec_for_arg(&self.args[arg_idx], style, backend)?);
            arg_idx += 1;
        }

        Ok(LoweredFormat {
            backend,
            format_string: rendered,
        })
    }
}

fn parse_template(template: &str) -> Result<Vec<PlaceholderStyle>, LowerError> {
    let mut styles = Vec::new();
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '{' {
            continue;
        }

        let mut spec = String::new();
        for next in chars.by_ref() {
            if next == '}' {
                break;
            }
            spec.push(next);
        }
        styles.push(parse_placeholder(&spec)?);
    }

    Ok(styles)
}

fn parse_placeholder(spec: &str) -> Result<PlaceholderStyle, LowerError> {
    match spec {
        "" => Ok(PlaceholderStyle {
            kind: PlaceholderKind::Default,
            alternate: false,
            zero_pad_width: None,
        }),
        ":x" => Ok(PlaceholderStyle {
            kind: PlaceholderKind::LowerHex,
            alternate: false,
            zero_pad_width: None,
        }),
        ":X" => Ok(PlaceholderStyle {
            kind: PlaceholderKind::UpperHex,
            alternate: false,
            zero_pad_width: None,
        }),
        ":#x" => Ok(PlaceholderStyle {
            kind: PlaceholderKind::LowerHex,
            alternate: true,
            zero_pad_width: None,
        }),
        ":#X" => Ok(PlaceholderStyle {
            kind: PlaceholderKind::UpperHex,
            alternate: true,
            zero_pad_width: None,
        }),
        ":p" => Ok(PlaceholderStyle {
            kind: PlaceholderKind::Pointer,
            alternate: false,
            zero_pad_width: None,
        }),
        _ => parse_zero_padded_hex(spec),
    }
}

fn parse_zero_padded_hex(spec: &str) -> Result<PlaceholderStyle, LowerError> {
    let body = spec
        .strip_prefix(':')
        .ok_or_else(|| LowerError::UnsupportedSpecifier(spec.to_owned()))?;

    let (alternate, body) = if let Some(rest) = body.strip_prefix('#') {
        (true, rest)
    } else {
        (false, body)
    };

    let body = body
        .strip_prefix('0')
        .ok_or_else(|| LowerError::UnsupportedSpecifier(spec.to_owned()))?;

    if body.len() < 2 {
        return Err(LowerError::UnsupportedSpecifier(spec.to_owned()));
    }

    let (digits, suffix) = body.split_at(body.len() - 1);
    let width = digits
        .parse::<usize>()
        .map_err(|_| LowerError::UnsupportedSpecifier(spec.to_owned()))?;

    let kind = match suffix {
        "x" => PlaceholderKind::LowerHex,
        "X" => PlaceholderKind::UpperHex,
        _ => return Err(LowerError::UnsupportedSpecifier(spec.to_owned())),
    };

    Ok(PlaceholderStyle {
        kind,
        alternate,
        zero_pad_width: Some(width),
    })
}

fn spec_for_arg(
    arg: &LogArg<'_>,
    style: PlaceholderStyle,
    backend: Backend,
) -> Result<String, LowerError> {
    match arg {
        LogArg::Ptr(_) => {
            if style.kind == PlaceholderKind::Pointer
                && !style.alternate
                && style.zero_pad_width.is_none()
            {
                Ok("%p".to_owned())
            } else {
                Err(LowerError::InvalidPointerStyle)
            }
        }
        LogArg::Str(_) => {
            if style.kind == PlaceholderKind::Default
                && !style.alternate
                && style.zero_pad_width.is_none()
            {
                Ok("%s".to_owned())
            } else {
                Err(LowerError::UnsupportedSpecifier(format!("{style:?}")))
            }
        }
        LogArg::Bool(_) => {
            if style.kind == PlaceholderKind::Default
                && !style.alternate
                && style.zero_pad_width.is_none()
            {
                Ok("%d".to_owned())
            } else {
                Err(LowerError::UnsupportedSpecifier(format!("{style:?}")))
            }
        }
        LogArg::U8(_) | LogArg::U16(_) | LogArg::U32(_) => integer_spec("u", style),
        LogArg::I8(_) | LogArg::I16(_) | LogArg::I32(_) => integer_spec("d", style),
        LogArg::U64(_) => integer_spec("llu", style),
        LogArg::I64(_) => integer_spec("lld", style),
        LogArg::Usize(_) => match backend {
            Backend::Windows => integer_spec("Iu", style),
            _ => integer_spec("zu", style),
        },
        LogArg::Isize(_) => match backend {
            Backend::Windows => integer_spec("Id", style),
            _ => integer_spec("zd", style),
        },
    }
}

fn integer_spec(base: &str, style: PlaceholderStyle) -> Result<String, LowerError> {
    match style.kind {
        PlaceholderKind::Default => {
            if style.alternate || style.zero_pad_width.is_some() {
                Err(LowerError::UnsupportedSpecifier(format!("{style:?}")))
            } else {
                Ok(format!("%{base}"))
            }
        }
        PlaceholderKind::LowerHex => Ok(compose_hex_spec(base, false, style.alternate, style.zero_pad_width)),
        PlaceholderKind::UpperHex => Ok(compose_hex_spec(base, true, style.alternate, style.zero_pad_width)),
        PlaceholderKind::Pointer => Err(LowerError::UnsupportedSpecifier(format!("{style:?}"))),
    }
}

fn compose_hex_spec(base: &str, upper: bool, alternate: bool, zero_pad_width: Option<usize>) -> String {
    let conv = if upper { 'X' } else { 'x' };
    let mut spec = String::from("%");
    if alternate {
        spec.push('#');
    }
    if let Some(width) = zero_pad_width {
        spec.push('0');
        spec.push_str(&width.to_string());
    }

    if let Some(length) = base.strip_suffix('u').or_else(|| base.strip_suffix('d')) {
        spec.push_str(length);
        spec.push(conv);
    } else {
        spec.push_str(base);
        spec.push(conv);
    }

    spec
}
