//! Downloads TPTP problems and translates them to rsbdd formulas for `rsbdd --prove`.

pub mod download;
pub mod tptp;
pub mod translate;

/// The lexer and parser generated from grammar/TPTP.g4 by the build script
#[allow(
    dead_code,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::restriction
)]
mod generated {
    pub mod tptp_lexer {
        include!(concat!(env!("OUT_DIR"), "/tptp_lexer.rs"));
    }
    pub mod tptp_parser {
        include!(concat!(env!("OUT_DIR"), "/tptp_parser.rs"));
    }
}
