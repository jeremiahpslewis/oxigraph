//! Oxigraph is a graph database library implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.
//!
//! Its goal is to provide a compliant, safe and fast on-disk database.
//! It also provides a set of utility functions for reading, writing, and processing RDF files.
//!
//! Oxigraph is in heavy development and SPARQL query evaluation has not been optimized yet.
//!
//! The disabled by default `"sophia"` feature provides [`sophia_api`](https://docs.rs/sophia_api/) traits implemention on Oxigraph terms and stores.
//!
//! Oxigraph also provides [a standalone HTTP server](https://crates.io/crates/oxigraph_server) based on this library.
//!
//!
//! The main entry point of Oxigraph is the [`Store`](store::Store) struct:
//!
//! ```
//! use oxigraph::store::Store;
//! use oxigraph::model::*;
//! use oxigraph::sparql::QueryResults;
//!
//! let store = Store::new()?;
//!
//! // insertion
//! let ex = NamedNode::new("http://example.com")?;
//! let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
//! store.insert(&quad)?;
//!
//! // quad filter
//! let results = store.quads_for_pattern(Some(ex.as_ref().into()), None, None, None).collect::<Result<Vec<Quad>,_>>()?;
//! assert_eq!(vec![quad], results);
//!
//! // SPARQL query
//! if let QueryResults::Solutions(mut solutions) =  store.query("SELECT ?s WHERE { ?s ?p ?o }")? {
//!     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
//! }
//! # Result::<_,Box<dyn std::error::Error>>::Ok(())
//! ```
#![deny(
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_qualifications
)]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/master/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/master/logo.svg")]
#![warn(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::checked_conversions,
    clippy::cloned_instead_of_copied,
    clippy::copy_iterator,
    clippy::dbg_macro,
    clippy::debug_assert_with_mut_call,
    clippy::decimal_literal_representation,
    //TODO clippy::doc_markdown,
    // clippy::else_if_without_else,
    clippy::empty_line_after_outer_attr,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::expect_used,
    clippy::expl_impl_clone_on_copy,
    clippy::explicit_deref_methods,
    clippy::explicit_into_iter_loop,
    clippy::explicit_iter_loop,
    clippy::fallible_impl_from,
    clippy::filter_map_next,
    clippy::flat_map_option,
    clippy::from_iter_instead_of_collect,
    clippy::get_unwrap,
    clippy::if_not_else,
    // clippy::if_then_some_else_none,
    clippy::implicit_clone,
    clippy::implicit_saturating_sub,
    clippy::imprecise_flops,
    clippy::inconsistent_struct_constructor,
    // clippy::indexing_slicing,
    clippy::inefficient_to_string,
    clippy::inline_always,
    clippy::invalid_upcast_comparisons,
    clippy::items_after_statements,
    clippy::large_digit_groups,
    clippy::large_stack_arrays,
    clippy::large_types_passed_by_value,
    clippy::let_underscore_drop,
    clippy::let_underscore_must_use,
    clippy::let_unit_value,
    clippy::linkedlist,
    clippy::macro_use_imports,
    clippy::manual_ok_or,
    //TODO clippy::map_err_ignore,
    clippy::map_flatten,
    clippy::map_unwrap_or,
    clippy::match_bool,
    // clippy::match_on_vec_items,
    clippy::match_same_arms,
    clippy::match_wildcard_for_single_variants,
    clippy::maybe_infinite_iter,
    clippy::mem_forget,
    //TODO clippy::missing_const_for_fn,
    //TODO clippy::module_name_repetitions,
    clippy::multiple_crate_versions,
    clippy::multiple_inherent_impl,
    //TODO clippy::must_use_candidate,
    clippy::mut_mut,
    clippy::mutex_integer,
    clippy::naive_bytecount,
    clippy::needless_bitwise_bool,
    clippy::needless_continue,
    clippy::needless_pass_by_value,
    clippy::non_ascii_literal,
    //TODO clippy::nonstandard_macro_braces,
    //TODO clippy::option_if_let_else,
    // clippy::panic, clippy::panic_in_result_fn, does not work well with tests
    clippy::path_buf_push_overwrite,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::range_minus_one,
    clippy::range_plus_one,
    //TODO clippy::rc_mutex,
    clippy::enum_variant_names,
    //TODO clippy::redundant_closure_for_method_calls,
    clippy::redundant_else,
    clippy::redundant_pub_crate,
    clippy::ref_binding_to_reference,
    clippy::ref_option_ref,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_functions_in_if_condition,
    // clippy::shadow_reuse,
    // clippy::shadow_same,
    // clippy::shadow_unrelated,
    // clippy::single_match_else,
    clippy::str_to_string,
    clippy::string_add,
    clippy::string_add_assign,
    clippy::string_lit_as_bytes,
    clippy::string_to_string,
    clippy::suboptimal_flops,
    clippy::suspicious_operation_groupings,
    clippy::todo,
    clippy::trait_duplication_in_bounds,
    clippy::transmute_ptr_to_ptr,
    clippy::trivial_regex,
    clippy::trivially_copy_pass_by_ref,
    clippy::type_repetition_in_bounds,
    clippy::unicode_not_nfc,
    clippy::unimplemented,
    clippy::unnecessary_self_imports,
    clippy::unnecessary_wraps,
    clippy::unneeded_field_pattern,
    clippy::unnested_or_patterns,
    clippy::unreadable_literal,
    clippy::unseparated_literal_suffix,
    clippy::unused_async,
    clippy::unused_self,
    clippy::use_debug,
    clippy::use_self,
    clippy::used_underscore_binding,
    clippy::useless_let_if_seq,
    clippy::useless_transmute,
    clippy::verbose_bit_mask,
    clippy::verbose_file_reads,
    clippy::wildcard_dependencies,
    clippy::zero_sized_map_values,
    clippy::wrong_self_convention,
)]
#![doc(test(attr(deny(warnings))))]

mod error;
pub mod io;
pub mod model;
#[cfg(feature = "sophia")]
mod sophia;
pub mod sparql;
mod storage;
pub mod store;
