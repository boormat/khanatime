// pub mod khana_rule;
// pub use khana_rule::RULES_MARKDOWN;

// Vec of str to strings.
// let a= stringify(["a", "b", "c"]);
pub fn stringify<const N: usize>(a: [&str; N]) -> [String; N] {
    // https://stackoverflow.com/a/67651639/364875
    a.map(String::from)
}
