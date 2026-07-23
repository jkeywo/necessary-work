//! Authored content: schemas, loading, and validation for *The Necessary Work*.
//!
//! Projects and scenario data are human-reviewable structured data (RON) with
//! stable ids, explicit units and scopes, and source metadata, embedded at
//! compile time so every build ships byte-identical content. Rust owns the
//! runtime rules; this crate owns the *shape* of what is authored and the
//! checks that keep it well-formed. It depends on nothing else in the
//! workspace so the simulation can consume it without cycles.

pub mod hash;
pub mod schema;
pub mod world;

pub use schema::*;
pub use world::{Continent, Icon, Sector};

/// The embedded authored catalogue sources.
pub const SCENARIO_RON: &str = include_str!("../../../content/scenario.ron");
pub const PROJECTS_RON: &str = include_str!("../../../content/projects.ron");

/// A cross-reference or schema issue found in authored content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentIssue {
    DuplicateId(String),
    BadId(String),
    UnresolvedReference { from: String, to: String },
    LockedWithoutUnlocker(String),
    BadScope { id: String, detail: String },
    BadValue { id: String, detail: String },
}

/// The loaded authored catalogue. `content_version` hashes the authored bytes,
/// so any replay-relevant content change changes the version.
#[derive(Debug, Clone)]
pub struct Catalogue {
    pub scenario: Scenario,
    pub projects: Vec<ProjectDef>,
    pub opportunities: Vec<OpportunityDef>,
    pub content_version: u64,
}

impl Catalogue {
    /// Parse the embedded content. Errors are formatted RON parse failures.
    pub fn load() -> Result<Catalogue, String> {
        let scenario_file: ScenarioFile =
            ron::from_str(SCENARIO_RON).map_err(|e| format!("scenario.ron: {e}"))?;
        let projects_file: ProjectsFile =
            ron::from_str(PROJECTS_RON).map_err(|e| format!("projects.ron: {e}"))?;
        let mut version_bytes = Vec::with_capacity(SCENARIO_RON.len() + PROJECTS_RON.len());
        version_bytes.extend_from_slice(SCENARIO_RON.as_bytes());
        version_bytes.extend_from_slice(PROJECTS_RON.as_bytes());
        Ok(Catalogue {
            scenario: scenario_file.scenario,
            projects: projects_file.projects,
            opportunities: scenario_file.opportunities,
            content_version: hash::fnv1a64(&version_bytes),
        })
    }

    /// The embedded catalogue, which must parse — enforced by test and CI.
    pub fn embedded() -> Catalogue {
        Catalogue::load().expect("embedded content must parse")
    }

    pub fn project_index(&self, id: &str) -> Option<usize> {
        self.projects.iter().position(|p| p.id == id)
    }

    /// Cross-reference validation. Returns every issue found so the content
    /// linter can report them all at once.
    pub fn validate(&self) -> Vec<ContentIssue> {
        let mut issues = Vec::new();
        let ids: Vec<&str> = self.projects.iter().map(|p| p.id.as_str()).collect();

        let mut seen = std::collections::BTreeSet::new();
        for id in &ids {
            if !seen.insert(*id) {
                issues.push(ContentIssue::DuplicateId((*id).to_string()));
            }
            if !is_kebab(id) {
                issues.push(ContentIssue::BadId((*id).to_string()));
            }
        }

        let exists = |target: &str| ids.contains(&target);
        let mut unlocked_targets = std::collections::BTreeSet::new();

        for project in &self.projects {
            for prerequisite in &project.prerequisites {
                let target = match prerequisite {
                    Prerequisite::CompletedOnLead { project } => Some(project),
                    Prerequisite::CompletedAnywhere { project } => Some(project),
                    Prerequisite::IconAtLeast { .. } => None,
                };
                if let Some(target) = target {
                    if !exists(target) {
                        issues.push(ContentIssue::UnresolvedReference {
                            from: project.id.clone(),
                            to: target.clone(),
                        });
                    }
                }
            }
            for effect in &project.effects {
                let target = match &effect.op {
                    EffectOp::ModifyProjectCost { project, .. } => Some(project),
                    EffectOp::ModifyProjectDuration { project, .. } => Some(project),
                    EffectOp::UnlockProject { project } => {
                        unlocked_targets.insert(project.clone());
                        Some(project)
                    }
                    _ => None,
                };
                if let Some(target) = target {
                    if !exists(target) {
                        issues.push(ContentIssue::UnresolvedReference {
                            from: project.id.clone(),
                            to: target.clone(),
                        });
                    }
                }
                // Spatial ops need a spatial scope; economy-wide ops are Global.
                let spatial_scope = !matches!(effect.scope, Scope::Global);
                if effect.op.is_spatial() != spatial_scope {
                    issues.push(ContentIssue::BadScope {
                        id: project.id.clone(),
                        detail: format!("{:?} with scope {:?}", effect.op, effect.scope),
                    });
                }
            }
            match project.repeat {
                Repeat::Tiered { count: 0 } => issues.push(ContentIssue::BadValue {
                    id: project.id.clone(),
                    detail: "Tiered count must be at least 1".into(),
                }),
                Repeat::CappedRollout { per_continent: 0 } => issues.push(ContentIssue::BadValue {
                    id: project.id.clone(),
                    detail: "CappedRollout per_continent must be at least 1".into(),
                }),
                _ => {}
            }
            if project.duration_ticks == 0 || project.finance_cost < 0 || project.mandate_cost < 0 {
                issues.push(ContentIssue::BadValue {
                    id: project.id.clone(),
                    detail: "duration must be positive; costs must be non-negative".into(),
                });
            }
        }

        for project in &self.projects {
            if project.locked && !unlocked_targets.contains(&project.id) {
                issues.push(ContentIssue::LockedWithoutUnlocker(project.id.clone()));
            }
        }

        // Recoverability invariants: strictly positive baseline incomes, and
        // exactly nine authored sector baselines.
        let scenario = &self.scenario;
        if scenario
            .finance_income_milli
            .iter()
            .map(|(_, v)| v)
            .sum::<i64>()
            <= 0
            || scenario
                .mandate_income_milli
                .iter()
                .map(|(_, v)| v)
                .sum::<i64>()
                <= 0
        {
            issues.push(ContentIssue::BadValue {
                id: scenario.id.clone(),
                detail: "baseline Finance and Mandate income must be positive".into(),
            });
        }
        if scenario.sector_baselines_milli_gt.len() != 9 {
            issues.push(ContentIssue::BadValue {
                id: scenario.id.clone(),
                detail: "exactly nine sector baselines are required".into(),
            });
        }

        issues
    }
}

fn is_kebab(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalogue_parses_and_validates() {
        let catalogue = Catalogue::embedded();
        let issues = catalogue.validate();
        assert!(issues.is_empty(), "content issues: {issues:?}");
        assert!(!catalogue.projects.is_empty());
        assert!(!catalogue.opportunities.is_empty());
    }

    #[test]
    fn content_version_is_stable_across_loads() {
        assert_eq!(
            Catalogue::embedded().content_version,
            Catalogue::embedded().content_version
        );
    }
}
