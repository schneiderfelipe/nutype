pub mod error;
pub mod tests;
pub mod traits;

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    common::{
        gen::{
            error::gen_error_type_name, tests::gen_test_should_have_valid_default_value,
            traits::GeneratedTraits, GenerateNewtype,
        },
        models::{ErrorTypeName, Guard, TypeName},
    },
    string::models::{RegexDef, StringInnerType, StringSanitizer, StringValidator},
};

use self::{error::gen_validation_error_type, traits::gen_traits};

use super::{
    models::{StringDeriveTrait, StringGuard},
    StringNewtype,
};

impl GenerateNewtype for StringNewtype {
    type Sanitizer = StringSanitizer;
    type Validator = StringValidator;
    type InnerType = StringInnerType;
    type TypedTrait = StringDeriveTrait;

    // For String based types, parse error is the same as validation error.
    const HAS_DEDICATED_PARSE_ERROR: bool = false;

    // With this `::new()` function receives `impl Into<String>` instead of `String`.
    // This allows to use &str with it.
    const NEW_CONVERT_INTO_INNER_TYPE: bool = true;

    fn gen_fn_sanitize(
        _inner_type: &Self::InnerType,
        sanitizers: &[Self::Sanitizer],
    ) -> TokenStream {
        let transformations: TokenStream = sanitizers
            .iter()
            .map(|san| match san {
                StringSanitizer::Trim => {
                    // TODO: consider optimizing sequences of [trim, lowercase] and [trim, uppercase] to avoid
                    // unnecessary allocation with `to_string()`
                    quote!(
                        let value: String = value.trim().to_string();
                    )
                }
                StringSanitizer::Lowercase => {
                    quote!(
                        let value: String = value.to_lowercase();
                    )
                }
                StringSanitizer::Uppercase => {
                    quote!(
                        let value: String = value.to_uppercase();
                    )
                }
                StringSanitizer::With(typed_custom_function) => {
                    quote!(
                        let value: String = (#typed_custom_function)(value);
                    )
                }
            })
            .collect();

        quote!(
            fn __sanitize__(value: String) -> String {
                #transformations
                value
            }
        )
    }

    fn gen_fn_validate(
        _inner_type: &Self::InnerType,
        type_name: &TypeName,
        validators: &[Self::Validator],
    ) -> TokenStream {
        let error_name = gen_error_type_name(type_name);

        // Indicates that `chars_count` variable needs to be set, which is used within
        // min_len and max_len validations.
        let mut requires_chars_count = false;

        let validations: TokenStream = validators
            .iter()
            .map(|validator| match validator {
                StringValidator::LenCharMax(max_len) => {
                    requires_chars_count = true;
                    quote!(
                        if chars_count > #max_len {
                            return Err(#error_name::LenCharMaxViolated);
                        }
                    )
                }
                StringValidator::LenCharMin(min_len) => {
                    requires_chars_count = true;
                    quote!(
                        if chars_count < #min_len {
                            return Err(#error_name::LenCharMinViolated);
                        }
                    )
                }
                StringValidator::NotEmpty => {
                    quote!(
                        if val.is_empty() {
                            return Err(#error_name::NotEmptyViolated);
                        }
                    )
                }
                StringValidator::Predicate(typed_custom_function) => {
                    quote!(
                        if !(#typed_custom_function)(&val) {
                            return Err(#error_name::PredicateViolated);
                        }
                    )
                }
                StringValidator::Regex(regex_def) => {
                    match regex_def {
                        RegexDef::StringLiteral(regex_str_lit) => {
                            quote!(
                                lazy_static::lazy_static! {
                                    // Make up a sufficiently unique regex name to ensure that it does
                                    // not clashes with anything import with `use super::*`.
                                    static ref __NUTYPE_REGEX__: ::regex::Regex = ::regex::Regex::new(#regex_str_lit).expect("Nutype failed to a build a regex");
                                }
                                if !__NUTYPE_REGEX__.is_match(&val) {
                                    return Err(#error_name::RegexViolated);
                                }
                            )

                        }
                        RegexDef::Path(regex_path) => {
                            quote!(
                                if !#regex_path.is_match(&val) {
                                    return Err(#error_name::RegexViolated);
                                }
                            )
                        }
                    }
                }
            })
            .collect();

        let chars_count_if_required = if requires_chars_count {
            quote!(
                let chars_count = val.chars().count();
            )
        } else {
            quote!()
        };

        quote!(
            fn __validate__(val: &str) -> ::core::result::Result<(), #error_name> {
                #chars_count_if_required
                #validations
                Ok(())
            }
        )
    }

    fn gen_validation_error_type(
        type_name: &TypeName,
        validators: &[Self::Validator],
    ) -> TokenStream {
        gen_validation_error_type(type_name, validators)
    }

    fn gen_traits(
        type_name: &TypeName,
        _inner_type: &Self::InnerType,
        maybe_error_type_name: Option<ErrorTypeName>,
        traits: HashSet<Self::TypedTrait>,
        maybe_default_value: Option<syn::Expr>,
        guard: &StringGuard,
    ) -> Result<GeneratedTraits, syn::Error> {
        gen_traits(
            type_name,
            maybe_error_type_name,
            traits,
            maybe_default_value,
            guard,
        )
    }

    fn gen_tests(
        type_name: &TypeName,
        _inner_type: &Self::InnerType,
        maybe_default_value: &Option<syn::Expr>,
        guard: &Guard<Self::Sanitizer, Self::Validator>,
        _traits: &HashSet<Self::TypedTrait>,
    ) -> TokenStream {
        let test_len_char_min_vs_max = guard.validators().and_then(|validators| {
            tests::gen_test_should_have_consistent_len_char_boundaries(type_name, validators)
        });

        let test_valid_default_value = gen_test_should_have_valid_default_value(
            type_name,
            maybe_default_value,
            guard.has_validation(),
        );

        quote! {
            #test_len_char_min_vs_max
            #test_valid_default_value
        }
    }
}
