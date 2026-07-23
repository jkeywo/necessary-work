//! Authored content: schemas, loading, and validation for *The Necessary Work*.
//!
//! Projects and scenario data are human-reviewable structured data (RON) with
//! stable ids, explicit units and scopes, and source metadata. Rust owns the
//! runtime rules; this crate owns the *shape* of what is authored and the
//! checks that keep it well-formed. It deliberately depends on nothing else in
//! the workspace so the simulation can consume it without cycles.
//!
//! This is a scaffold: the controlled effect vocabulary, project schema, and
//! linting are stubbed here and grown per the simulation contract.

/// Result of validating an authored content catalogue against its schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentIssue {
    /// A referenced id (project prerequisite, unlock, scope target) does not resolve.
    UnresolvedReference { from: String, to: String },
    /// A project declares an effect scope that is not one of the authored scopes.
    UnknownScope { project: String, scope: String },
}

/// The authored catalogue. Populated by loading and cross-reference validation.
#[derive(Debug, Clone, Default)]
pub struct Catalogue {
    /// Content version, bumped when authored data changes in a replay-relevant way.
    pub content_version: u32,
}

impl Catalogue {
    /// Cross-reference validation entry point. Returns every issue found so the
    /// content linter can report them all at once rather than failing on the first.
    pub fn validate(&self) -> Vec<ContentIssue> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_catalogue_is_valid() {
        assert!(Catalogue::default().validate().is_empty());
    }
}
