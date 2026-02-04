//! Decision stream types used for replay and scheduling.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Decision {
    /// Binary decision (predicate branch).
    Bool(bool),
    /// N-ary decision (e.g. async scheduling choice among `arity` options).
    Choice { arity: u32, index: u32 },
}

impl Decision {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Decision::Bool(b) => Some(*b),
            _ => None,
        }
    }
}
