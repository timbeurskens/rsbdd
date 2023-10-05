use std::{
    fmt::{self, Debug, Display},
    hash::{Hash, Hasher},
    rc::Rc,
};

pub trait BDDSymbol: Ord + Display + Debug + Clone + Hash {}

impl<T> BDDSymbol for T where T: Ord + Display + Debug + Clone + Hash {}

#[derive(Debug, Clone)]
pub struct NamedSymbol {
    pub name: Rc<String>,
    pub id: usize,
}

impl fmt::Display for NamedSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.name, f)
    }
}

impl Hash for NamedSymbol {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for NamedSymbol {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for NamedSymbol {}

impl Ord for NamedSymbol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for NamedSymbol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<NamedSymbol> for usize {
    fn from(ns: NamedSymbol) -> Self {
        ns.id
    }
}
