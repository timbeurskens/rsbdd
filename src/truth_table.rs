use std::{
    fmt::{self, Display},
    str::FromStr,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Single variable assignment in a truth table.
///
/// The variable assignments in a truth table can be one of True, False or Any.
/// [`True`] is assigned when the variable can only be assigned a 'true' value;
/// [`False`] is assigned when the variable can only be 'false'.
/// When the variable can either be true or false, the truth table can either consist of
/// both options (as separate models), or assign [`Any`].
///
/// [`Any`]: TruthTableEntry::Any
/// [`True`]: TruthTableEntry::True
/// [`False`]: TruthTableEntry::False
pub enum TruthTableEntry {
    /// Assigned when the variable can only be true
    True,
    /// Assigned when the variable can only be false
    False,
    /// Assigned when the variable can either be true or false
    Any,
    /// Assigned when the variable can neither be true or false
    None,
}

impl TruthTableEntry {
    const fn variants<'a>() -> &'a [Self] {
        &[Self::True, Self::False, Self::Any, Self::None]
    }

    fn matches(&self, s: &str) -> bool {
        match self {
            Self::True => matches!(s, "true" | "True" | "t" | "T" | "1"),
            Self::False => matches!(s, "false" | "False" | "f" | "F" | "0"),
            Self::Any => matches!(s, "any" | "Any" | "a" | "A" | "*"),
            Self::None => matches!(s, "none" | "None" | "n" | "N" | "-"),
        }
    }

    pub const fn is_true(self) -> bool {
        matches!(self, Self::True)
    }

    pub const fn is_false(self) -> bool {
        matches!(self, Self::False)
    }

    pub const fn is_any(self) -> bool {
        matches!(self, Self::Any)
    }

    pub const fn is_none(self) -> bool {
        matches!(self, Self::None)
    }
}

impl Display for TruthTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(match self {
            Self::True => "True",
            Self::False => "False",
            Self::Any => "Any",
            Self::None => "None",
        })
    }
}

impl FromStr for TruthTableEntry {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::variants()
            .iter()
            .find(|variant| variant.matches(s))
            .ok_or_else(|| anyhow::anyhow!("cannot parse {s} as truth-table entry"))
            .copied()
    }
}
