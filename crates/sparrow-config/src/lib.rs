// Foundation layer extracted from the sparrow-cli monolith: configuration, the
// provider registry, credential store, permissions, lifecycle hooks, sandbox
// backends and the humanize layer. Depends only on sparrow-core + providers.
//
// Same lint policy as the main crate (this code was extracted from it).
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::derivable_impls,
    clippy::format_in_format_args,
    clippy::if_same_then_else,
    clippy::iter_cloned_collect,
    clippy::manual_clamp,
    clippy::manual_div_ceil,
    clippy::manual_is_multiple_of,
    clippy::manual_pattern_char_comparison,
    clippy::needless_borrow,
    clippy::needless_range_loop,
    clippy::new_without_default,
    clippy::ptr_arg,
    clippy::should_implement_trait,
    clippy::single_match,
    clippy::type_complexity,
    clippy::unnecessary_cast,
    clippy::let_and_return,
    clippy::useless_conversion,
    clippy::useless_format,
    clippy::while_let_loop
)]

pub mod auth;
pub mod config;
pub mod hooks;
pub mod humanize;
pub mod permissions;
pub mod sandbox;
