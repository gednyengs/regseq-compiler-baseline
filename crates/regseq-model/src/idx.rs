//! Typed-ID infrastructure for RegseqModel.
//!
//! Defines the `Idx` trait, a private niche-bearing `u32` wrapper
//! (`NonMaxU32`), and the `newtype_index!` macro that generates each
//! concrete ID type. Per the stage-1 spec, the macro does not emit
//! `Serialize` / `Deserialize` -- those are written separately
//! alongside each ID's invocation in `crate::ids`.

use core::fmt::Debug;
use core::hash::Hash;

/// Bidirectional conversion between a typed ID and its logical
/// `usize` index.
///
/// All RegseqModel IDs implement this trait. They are post-bridge,
/// deterministically assigned, and `Ord` is total over the logical
/// index. `'static` makes the model's owned/`'tcx`-free IR
/// invariant explicit at the trait boundary (see Q2 in
/// `architecture.md`); all stage-1 IDs are `u32`-backed and
/// naturally `'static`.
///
/// `Idx::new` panics if the value exceeds the implementation's
/// `MAX`. Each ID newtype additionally exposes an inherent
/// `pub const fn from_u32` with the same panic semantics for use
/// in const contexts.
pub trait Idx: Copy + Eq + Ord + Hash + Debug + 'static {
    fn new(idx: usize) -> Self;
    fn index(self) -> usize;
}

/// Private niche-bearing `u32` wrapper. Reserves `u32::MAX` as the
/// niche; every other `u32` is representable. Used as the inner
/// field of every `newtype_index!`-generated ID so that
/// `Option<Id>` is laid out in 4 bytes.
///
/// Encoding: stored value is `logical ^ u32::MAX`. The `NonZeroU32`
/// invariant holds because logical `<= u32::MAX - 1` implies stored
/// `>= 1`. The XOR is its own inverse, so encode/decode are
/// symmetric.
///
/// Auto-derived `PartialOrd`/`Ord` on this type would compare the
/// encoded form, which flips the relation relative to logical
/// indices. The macro therefore hand-writes ordering on each ID
/// against `as_u32()`; this type itself does not derive ordering.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NonMaxU32(core::num::NonZeroU32);

impl NonMaxU32 {
    pub(crate) const MAX_VALUE: u32 = u32::MAX - 1;

    /// # Safety
    /// `value` must be `<= MAX_VALUE` (i.e., not `u32::MAX`).
    #[inline]
    pub(crate) const unsafe fn new_unchecked(value: u32) -> Self {
        // SAFETY: caller guarantees `value <= MAX_VALUE`, so
        // `value ^ u32::MAX` is in `[1, u32::MAX]`.
        unsafe { Self(core::num::NonZeroU32::new_unchecked(value ^ u32::MAX)) }
    }

    #[inline]
    pub(crate) const fn get(self) -> u32 {
        self.0.get() ^ u32::MAX
    }
}

/// Generates a typed ID newtype with niche-encoded `u32` backing,
/// inherent const constructors/accessors, an `Idx` impl, manual
/// `Ord`/`PartialOrd` over the logical index, and a `Debug` impl
/// rendering as `TypeName(N)`.
///
/// Per the stage-1 spec, this macro does *not* emit `Serialize` /
/// `Deserialize`. Each ID's serde impl lives next to its
/// invocation in `crate::ids`.
macro_rules! newtype_index {
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident {}
    ) => {
        $(#[$attr])*
        #[repr(transparent)]
        #[derive(Copy, Clone, PartialEq, Eq, Hash)]
        $vis struct $name {
            private_use_methods_instead: $crate::idx::NonMaxU32,
        }

        impl $name {
            pub const MAX_AS_U32: u32 = $crate::idx::NonMaxU32::MAX_VALUE;
            pub const MAX: Self = Self::from_u32(Self::MAX_AS_U32);
            pub const ZERO: Self = Self::from_u32(0);

            #[inline]
            pub const fn from_u32(idx: u32) -> Self {
                assert!(
                    idx <= Self::MAX_AS_U32,
                    concat!(stringify!($name), " value exceeds MAX")
                );
                Self {
                    // SAFETY: the assert above guarantees
                    // `idx <= NonMaxU32::MAX_VALUE`.
                    private_use_methods_instead: unsafe {
                        $crate::idx::NonMaxU32::new_unchecked(idx)
                    },
                }
            }

            #[inline]
            pub const fn as_u32(self) -> u32 {
                self.private_use_methods_instead.get()
            }

            #[inline]
            pub const fn index(self) -> usize {
                self.as_u32() as usize
            }
        }

        impl $crate::idx::Idx for $name {
            #[inline]
            fn new(idx: usize) -> Self {
                assert!(
                    idx <= Self::MAX_AS_U32 as usize,
                    concat!(stringify!($name), " value exceeds MAX")
                );
                Self::from_u32(idx as u32)
            }

            #[inline]
            fn index(self) -> usize {
                Self::index(self)
            }
        }

        impl ::core::cmp::PartialOrd for $name {
            #[inline]
            fn partial_cmp(
                &self,
                other: &Self,
            ) -> ::core::option::Option<::core::cmp::Ordering> {
                ::core::option::Option::Some(::core::cmp::Ord::cmp(self, other))
            }
        }

        impl ::core::cmp::Ord for $name {
            #[inline]
            fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                ::core::cmp::Ord::cmp(&self.as_u32(), &other.as_u32())
            }
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(
                &self,
                f: &mut ::core::fmt::Formatter<'_>,
            ) -> ::core::fmt::Result {
                write!(f, "{}({})", stringify!($name), self.as_u32())
            }
        }
    };
}

pub(crate) use newtype_index;
