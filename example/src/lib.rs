// Ensure only one backend is enabled
#[cfg(all(feature = "pure-rust", feature = "c-backend"))]
compile_error!("Enable exactly one backend: 'pure-rust' OR 'c-backend'.");

// Re-export modules that contain grammars
pub mod ambiguous;
pub mod ambiguous_expr;
pub mod arithmetic;
pub mod boolean_expr;
pub mod csv_list;
pub mod dangling_else;
pub mod external_word_example;
pub mod fielded_precedence_typed_cst_contract;
pub mod fielded_typed_cst_contract;
pub mod ini_file;
pub mod json_like;
pub mod lambda_calculus;
pub mod object_like_contract;
pub mod optionals;
pub mod performance_test;
pub mod reduce_reduce;
pub mod regex_grammar;
pub mod repetitions;
pub mod test_precedence;
pub mod test_whitespace;
pub mod typed_ast_contract;
pub mod words;

// Tree-sitter compatibility language helpers
#[cfg(all(feature = "ts-compat", feature = "pure-rust"))]
pub mod ts_langs;
