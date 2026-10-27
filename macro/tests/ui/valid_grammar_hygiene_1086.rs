//! Issue #1086 downstream witness (valid-grammar recipe, NOT compiled by lanes).
//!
//! This file invokes the real `#[adze::grammar]` proc macro (real compiler
//! spans, not fallback spans) over a grammar module that deliberately reuses
//! the macro's generated names:
//!
//! * a user type literally named `WithLeaf`, which the fallible wrapper
//!   rewrite must leave alone (only `adze::WithLeaf` may become
//!   `adze::WithTryLeaf`);
//! * a user function literally named `__adze_text` returning `i32`, which the
//!   generated `&str` closure parameter's hygiene must not capture.
//!
//! Before the fix this file fails to compile: the rewrite clobbered the user
//! `WithLeaf` path into a nonexistent `WithTryLeaf` (E0425), and the
//! call-site `__adze_text` parameter captured the user function (E0618).
//! After the fix it compiles and the assertions below pass.
//!
//! ## Why this lives here instead of as a test target
//!
//! An automated test target inside `adze-macro` would need a dev-dependency
//! from the proc-macro package onto the `adze` runtime. Under resolver v2
//! that edge splits feature resolution (`{default, pure-rust}` vs
//! `{pure-rust}` units for `adze`), duplicating `adze-glr-core` in combined
//! selections and breaking lanes with E0308/E0460-class errors. So this stays
//! an inert recipe: to run it, paste it into a scratch crate that depends on
//! `adze` (default-features off, tree-sitter expansion) and run
//! `cargo test`. Do NOT enable the macro's `pure-rust` feature there: the
//! pure-rust expansion `include!`s a build-generated parser that only exists
//! for real grammar crates.
//!
//! A `static`/`const` cannot serve as the adversarial vehicle: shadowing a
//! `static` with a parameter is a hard E0530 error with or without hygiene,
//! and a same-named `const` hijacks even hygienic binding patterns in pattern
//! position -- verified to also break `macro_rules!` (the `mixed_site` gold
//! standard) on this toolchain. Functions resolve purely by scope, which is
//! exactly what `mixed_site` governs.

use adze::Extract;

/// Downstream colliding type: shares its name with the `adze::WithLeaf`
/// leaf wrapper but is an ordinary user type. It lives outside the grammar
/// module so the macro emits no `Extract` impl for it; it is only named by a
/// leaf field type.
pub struct WithLeaf<T>(pub T);

/// Grammar exercising both soft spots from #1086.
///
/// The generated `language()`/`parse()` fns are intentionally uncalled (there
/// is no compiled tree-sitter parser for this scratch grammar); they only need
/// to type-check.
#[allow(dead_code)]
#[adze::grammar("hygiene1086")]
mod hygiene_grammar {
    use super::WithLeaf;

    // Adversarial user binding: same name as the generated fallible-closure
    // parameter, deliberately non-`&str` so that capture by the generated
    // binding is a type error instead of silent misbehavior.
    #[allow(dead_code)]
    fn __adze_text() -> i32 {
        41
    }

    #[adze::language]
    pub struct Root {
        #[adze::leaf(
            pattern = r"\d+",
            try_transform = |v: &str| Ok::<WithLeaf<String>, std::num::ParseIntError>(WithLeaf(
                v.to_owned()
            ))
        )]
        pub wrapped: WithLeaf<String>,
        #[adze::leaf(
            pattern = r"\d+",
            try_transform = |t: &str| t.parse::<i32>().map(|n| n + __adze_text())
        )]
        pub count: i32,
    }
}

fn assert_fallible_extract<T>()
where
    T: Extract<T, LeafFn = ()>,
{
}

#[test]
fn colliding_with_leaf_still_resolves_to_user_type() {
    // The macro must have kept the user path intact: this names the exact
    // type the fixed expansion generates for the `wrapped` field.
    fn assert_wrapped(_: Option<&adze::WithTryLeaf<WithLeaf<String>>>) {}
    assert_wrapped(None);

    // The grammar root is an ordinary struct we can construct directly.
    let root = hygiene_grammar::Root {
        wrapped: WithLeaf(String::from("7")),
        count: 1,
    };
    assert_eq!(root.wrapped.0, "7");
    assert_eq!(root.count, 1);
}

#[test]
fn generated_bindings_do_not_capture_user_bindings() {
    // The macro generated a fallible `Extract` impl for the root (proves the
    // `try_transform` path was taken). Reaching this point proves the user
    // `__adze_text` function was not captured by the generated `&str`
    // parameter.
    assert_fallible_extract::<hygiene_grammar::Root>();
}
