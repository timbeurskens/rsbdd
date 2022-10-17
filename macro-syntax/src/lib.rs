#[macro_export]
macro_rules! bdd {
    ($($expr:tt)+) => {{
        let input = stringify!($($expr)+);
        let mut input_reader = std::io::BufReader::new(input.as_bytes());
        let parsed_formula = rsbdd::parser::ParsedFormula::new(&mut input_reader, None).expect("could not parse expression");

        parsed_formula.eval()
    }};
}
