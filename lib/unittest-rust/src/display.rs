/*
 * Copyright (C) 2025 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::{
    any::type_name,
    fmt::{self, Debug, Display, Formatter},
};

/// Provides access to a best-effort implementation of [Display].
///
/// [BestEffortDisplay] creates a wrapper that implements [Display] for the
/// provided type.
///   * If `T` implements [Display], the wrapper will forward to `T`'s [Display]
///     implementation.
///   * If `T` does not implement [Display], but does implement [Debug], the
///     wrapper will use to `T`'s [Debug] implementation.
///   * If `T` implements neither [Display] nor [Debug], the wrapper will
///     display `T`'s [type_name].
///
/// The wrapper should be created with the following expression. The
/// [DisplayKind], [DebugKind], and [TypenameKind] traits need to be in scope.
/// (` as _` is fine; the traits just need to be in scope for method resolution;
/// the actual trait names don't need to be accessible.)
///
/// ```
/// # fn example() -> impl std::fmt::Display {
/// # let x: usize = 5;
/// # use test::__internal_macro_utils::{
/// #     BestEffortDisplay,
/// #     DisplayKind as _,
/// #     DebugKind as _,
/// #     TypenameKind as _,
/// # };
/// (&&&BestEffortDisplay(&x)).display_kind().wrap(x)
/// # }
/// ```
///
/// # Examples
///
/// ```
/// use test::__internal_macro_utils::BestEffortDisplay;
///
/// struct NeitherDisplayOrDebug;
/// #[derive(Debug)]
/// struct DebugOnlyType;
/// struct DisplayType;
///
/// impl std::fmt::Display for DisplayType {
///     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
///         write!(f, "Fancy Description")
///     }
/// }
///
/// let a = DisplayType;
/// let b = DebugOnlyType;
/// let c = NeitherDisplayOrDebug;
///
/// use test::__internal_macro_utils::{DisplayKind as _, DebugKind as _, TypenameKind as _};
/// let a = (&&&BestEffortDisplay(&a)).display_kind().wrap(a);
/// let b = (&&&BestEffortDisplay(&b)).display_kind().wrap(b);
/// let c = (&&&BestEffortDisplay(&c)).display_kind().wrap(c);
///
/// assert_eq!(a.to_string(), "Fancy Description");
/// assert_eq!(b.to_string(), "DebugOnlyType");
/// assert_eq!(c.to_string(),
///            format!("value of type {}",
///                    std::any::type_name::<NeitherDisplayOrDebug>()));
/// ```
///
/// [BestEffortDisplay] is implemented with [autoref specialization], which has
/// some limitations, namely that we have to know that `T` implements [Display]
/// at `get`'s callsite. As a result, it's mostly only useful inside macros.
/// Because the specialization has [more than two levels], we're acutually using
/// auto*de*ref, which is why the usage requires calling `display_kind` on
/// `&&&Self` (one more `&` than the highest priority `*Kind` impl).
///
/// Once stable Rust supports [min_specialization], this should be replaced to
/// use that instead.
///
/// [autoref specialization]: https://github.com/dtolnay/case-studies/blob/master/autoref-specialization/README.md
/// [more than two levels]: https://lukaskalbertodt.github.io/2019/12/05/generalized-autoref-based-specialization.html#using-autoderef-for--two-specialization-levels
/// [min_specialization]: https://doc.rust-lang.org/nightly/unstable-book/language-features/min-specialization.html#min_specialization
pub struct BestEffortDisplay<T>(pub T);

// When calling `(&&&BestEffortDisplay(&x)).display_kind()`, autoderef
// prioritizes the `DisplayKind` impl if it applies, then the `DebugKind` impl
// if it applies, then `TypenameKind`, which always applies.
impl<T: Display> DisplayKind for &&BestEffortDisplay<&T> {}
impl<T: Debug> DebugKind for &BestEffortDisplay<&T> {}
impl<T> TypenameKind for BestEffortDisplay<&T> {}

/// Constructs [Display] wrappers for types that implement [Display].
pub struct DisplayTag;
/// Constructs [Display] wrappers for types that implement [Debug].
pub struct DebugTag;
/// Constructs [Display] wrappers for any type.
pub struct TypenameTag;

/// Implements [Display] by delegating to `T`'s [Display] implementation.
pub struct DisplayDisplayer<T>(T);
/// Implements [Display] by delegating to `T`'s [Debug] implementation.
pub struct DebugDisplayer<T>(T);
/// Implements [Display] by printing `T`'s [type_name].
pub struct TypenameDisplayer<T>(T);

pub trait DisplayKind {
    #[inline]
    fn display_kind(&self) -> DisplayTag {
        DisplayTag
    }
}
pub trait DebugKind {
    #[inline]
    fn display_kind(&self) -> DebugTag {
        DebugTag
    }
}
pub trait TypenameKind {
    #[inline]
    fn display_kind(&self) -> TypenameTag {
        TypenameTag
    }
}

impl DisplayTag {
    pub fn wrap<T: Display>(self, t: T) -> DisplayDisplayer<T> {
        DisplayDisplayer(t)
    }
}
impl DebugTag {
    pub fn wrap<T: Debug>(self, t: T) -> DebugDisplayer<T> {
        DebugDisplayer(t)
    }
}
impl TypenameTag {
    pub fn wrap<T>(self, t: T) -> TypenameDisplayer<T> {
        TypenameDisplayer(t)
    }
}

impl<T: Display> Display for DisplayDisplayer<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl<T: Debug> Display for DebugDisplayer<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
impl<T> Display for TypenameDisplayer<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "value of type {}", type_name::<T>())
    }
}
