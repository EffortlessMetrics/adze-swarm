use adze_build_invalidation_fixture::grammar::{Word, parse};

fn main() {
    assert_eq!(
        parse("hello").expect("independent valid-input oracle"),
        Word { value: () }
    );
    println!("tiny-fixture-ok");
}
