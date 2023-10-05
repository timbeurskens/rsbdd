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
}

impl TruthTableEntry {
    fn variants<'a>() -> &'a [Self] {
        &[Self::True, Self::False, Self::Any]
    }

    fn matches(&self, s: &str) -> bool {
        match self {
            TruthTableEntry::True => matches!(s, "true" | "True" | "t" | "T" | "1"),
            TruthTableEntry::False => matches!(s, "false" | "False" | "f" | "F" | "0"),
            TruthTableEntry::Any => matches!(s, "any" | "Any" | "a" | "A" | "*"),
        }
    }

    pub fn is_true(self) -> bool {
        self == TruthTableEntry::True
    }

    pub fn is_false(self) -> bool {
        self == TruthTableEntry::False
    }

    pub fn is_any(self) -> bool {
        self == TruthTableEntry::Any
    }
}

impl Display for TruthTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(match self {
            TruthTableEntry::True => "True",
            TruthTableEntry::False => "False",
            TruthTableEntry::Any => "Any",
        })
    }
}

impl FromStr for TruthTableEntry {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::variants()
            .iter()
            .find(|variant| variant.matches(s))
            .ok_or(anyhow::anyhow!("cannot parse {s} as truth-table entry"))
            .copied()
    }
}
