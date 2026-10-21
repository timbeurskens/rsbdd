//! Generates the TPTP lexer and parser from the ANTLR grammar of the TPTP language, grammar/TPTP.g4, which is
//! https://tptp.org/UserDocs/TPTPLanguage/TPTP.g4 (v9.3.1.6), with the lexer rules for the parts of numbers made
//! fragments, so that integers are lexed as `Integer`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let generation = antlr_rust_codegen::Builder::new()
        .grammar("grammar/TPTP.g4")
        .entry_rule("tptp_file")
        .generate_listener(false)
        .out_dir(std::env::var_os("OUT_DIR").ok_or("Cargo sets OUT_DIR")?)
        .generate()?;
    generation.emit_rerun_if_changed();
    Ok(())
}
