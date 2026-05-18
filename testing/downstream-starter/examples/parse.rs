use downstream_starter::grammar;

fn main() {
    let source = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "1 + 2 * 3".to_string());

    match grammar::parse(&source) {
        Ok(expr) => println!("{expr:?}"),
        Err(errors) => {
            for error in &errors {
                eprintln!("{}", error.display_with_source(&source));
            }
            std::process::exit(1);
        }
    }
}
