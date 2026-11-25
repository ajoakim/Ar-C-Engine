//! Job priority classes.

/// Priority class; higher runs first. Declaration order defines `Ord`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    /// Lowest priority.
    Background,
    /// Low priority.
    Low,
    /// Default priority.
    Normal,
    /// High priority.
    High,
    /// Highest priority.
    Critical,
}

#[cfg(test)]
mod tests {
    use super::Priority::*;

    #[test]
    fn priority_ordering() {
        assert!(Critical > High && High > Normal && Normal > Low && Low > Background);
    }
}
