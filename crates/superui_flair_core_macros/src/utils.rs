use proc_macro_crate::{Error as CrateError, FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, quote};
use std::cell::LazyCell;

struct CratePath {
    pub crate_name: &'static str,
    pub path: Option<&'static str>,
    // >>> SUPERUI-FORK-PATCH: flair-macros-vendored-name  (docs/fork-patches.md#flair-macros-vendored-name)
    /// When `FoundCrate::Itself` is detected, emit this ident instead of `crate_name`.
    /// Used when the crate was vendored under a different lib name but keeps an
    /// `extern crate self as <original_name>;` alias so the path still resolves.
    pub itself_alias: Option<&'static str>,
    // <<< SUPERUI-FORK-PATCH: flair-macros-vendored-name
}

impl CratePath {
    fn new(crate_name: &'static str) -> Self {
        Self {
            crate_name,
            path: None,
            // >>> SUPERUI-FORK-PATCH: flair-macros-vendored-name  (docs/fork-patches.md#flair-macros-vendored-name)
            itself_alias: None,
            // <<< SUPERUI-FORK-PATCH: flair-macros-vendored-name
        }
    }

    fn with_path(crate_name: &'static str, path: &'static str) -> Self {
        Self {
            crate_name,
            path: Some(path),
            // >>> SUPERUI-FORK-PATCH: flair-macros-vendored-name  (docs/fork-patches.md#flair-macros-vendored-name)
            itself_alias: None,
            // <<< SUPERUI-FORK-PATCH: flair-macros-vendored-name
        }
    }

    // >>> SUPERUI-FORK-PATCH: flair-macros-vendored-name  (docs/fork-patches.md#flair-macros-vendored-name)
    /// Construct a candidate where `proc_macro_crate` looks up `crate_name` in Cargo.toml,
    /// but when `FoundCrate::Itself` fires (meaning we ARE that crate), the emitted path
    /// uses `itself_alias` instead — matching the `extern crate self as <itself_alias>;`
    /// declaration in the vendored crate's lib.rs.
    fn with_alias(crate_name: &'static str, itself_alias: &'static str) -> Self {
        Self {
            crate_name,
            path: None,
            itself_alias: Some(itself_alias),
        }
    }
    // <<< SUPERUI-FORK-PATCH: flair-macros-vendored-name

    fn resolve(self) -> Result<TokenStream, CrateError> {
        let found_crate = crate_name(self.crate_name)?;

        let maybe_path = self.path.map(|path| {
            let path_ident = Ident::new(path, Span::call_site());
            quote!(::#path_ident)
        });

        Ok(match found_crate {
            FoundCrate::Itself => {
                // This could be three cases:
                //  - bevy_flair examples/ -> We cannot use crate::core, we need to use `bevy_flair::core`.
                //  - bevy_flair_core docs -> We cannot use crate::, we need to use `bevy_flair_core`.
                //  - bevy_flair_core src/ -> We should use `crate`, but the previous case makes it not viable.
                //                            As a workaround we need to inject `extern crate self as my_crate_name;`.
                //                            but it's only internal code.
                // See https://github.com/bkchr/proc-macro-crate/issues/14
                // >>> SUPERUI-FORK-PATCH: flair-macros-vendored-name  (docs/fork-patches.md#flair-macros-vendored-name)
                // Use the itself_alias when set so that vendored-name crates (which declare
                // `extern crate self as bevy_flair_core;`) emit the original path rather
                // than the renamed lib name.
                let emit_name = self.itself_alias.unwrap_or(self.crate_name);
                let crate_name = Ident::new(emit_name, Span::call_site());
                // <<< SUPERUI-FORK-PATCH: flair-macros-vendored-name
                quote!( ::#crate_name #maybe_path )
            }
            FoundCrate::Name(name) => {
                let crate_name = Ident::new(&name, Span::call_site());
                quote!( ::#crate_name #maybe_path )
            }
        })
    }
}

fn resolve_first_available_crate(crates: impl IntoIterator<Item = CratePath>) -> TokenStream {
    let mut not_found_errors = Vec::new();

    for next in crates {
        match next.resolve() {
            Ok(v) => return v,
            Err(err @ CrateError::CrateNotFound { .. }) => {
                not_found_errors.push(err.to_string());
            }
            Err(e) => {
                panic!("{e}");
            }
        };
    }

    panic!("{}", not_found_errors.join("\n"));
}

fn bevy_ecs_path() -> TokenStream {
    resolve_first_available_crate([
        CratePath::with_path("bevy", "ecs"),
        CratePath::new("bevy_ecs"),
    ])
}

fn bevy_reflect_path() -> TokenStream {
    resolve_first_available_crate([
        CratePath::with_path("bevy", "reflect"),
        CratePath::new("bevy_reflect"),
    ])
}

fn bevy_flair_core_path() -> TokenStream {
    resolve_first_available_crate([
        CratePath::with_path("bevy_flair", "core"),
        CratePath::new("bevy_flair_core"),
        // >>> SUPERUI-FORK-PATCH: flair-macros-vendored-name  (docs/fork-patches.md#flair-macros-vendored-name)
        // Third candidate: the vendored fork is named `superui_flair_core` on crates.io but
        // declares `extern crate self as bevy_flair_core;` in its lib.rs, so `::bevy_flair_core`
        // resolves correctly inside the crate itself and for downstream dependents.
        CratePath::with_alias("superui_flair_core", "bevy_flair_core"),
        // <<< SUPERUI-FORK-PATCH: flair-macros-vendored-name
    ])
}

pub(crate) struct LazyTokens<T> {
    lazy: LazyCell<T>,
}

impl<T> LazyTokens<T> {
    pub fn new(f: fn() -> T) -> Self {
        Self {
            lazy: LazyCell::new(f),
        }
    }
}

impl<T> ToTokens for LazyTokens<T>
where
    T: ToTokens,
{
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.lazy.to_tokens(tokens);
    }
}

pub(crate) struct CratePaths {
    pub bevy_flair_core: LazyTokens<TokenStream>,
    pub bevy_ecs: LazyTokens<TokenStream>,
    pub bevy_reflect: LazyTokens<TokenStream>,
}

impl CratePaths {
    pub fn new() -> Self {
        Self {
            bevy_flair_core: LazyTokens::new(bevy_flair_core_path),
            bevy_ecs: LazyTokens::new(bevy_ecs_path),
            bevy_reflect: LazyTokens::new(bevy_reflect_path),
        }
    }
}
