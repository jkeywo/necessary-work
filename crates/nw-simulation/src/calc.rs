//! Scaling, breakpoints, and cost curves — with every calculation exposing its
//! base value, named modifiers in application order, and final value. Project
//! previews, completion records, and debug traces all use this same output, so
//! the explanation UI cannot diverge from the calculation.

use nw_content::schema::{Breakpoint, CostCurve, ProjectDef};
use nw_content::world::{Continent, Icon};
use serde::{Deserialize, Serialize};

/// One named modifier, in permille, in application order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifier {
    pub name: String,
    pub permille: i64,
}

/// A fully-exposed calculation: base, ordered modifiers, and the result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalcTrace {
    pub base: i64,
    pub modifiers: Vec<Modifier>,
    pub bonus_permille: i64,
    pub final_value: i64,
}

/// Apply a permille bonus to an integer base, rounding down.
pub fn apply_permille(base: i64, bonus_permille: i64) -> i64 {
    base * (1000 + bonus_permille) / 1000
}

/// The scaling+breakpoint bonus for a project's emissions reductions, given
/// the lead continent's current icons. Modifier order is documented and fixed:
/// scaling rules in authored order, then breakpoints in authored order.
pub struct ReductionBonus {
    pub modifiers: Vec<Modifier>,
    pub bonus_permille: i64,
    pub breakpoints_met: Vec<String>,
    /// The first authored breakpoint not yet met, if any.
    pub next_breakpoint: Option<Breakpoint>,
}

pub fn reduction_bonus(def: &ProjectDef, lead: Continent, icons: &[[i64; 4]; 3]) -> ReductionBonus {
    let lead_icons = &icons[lead.index()];
    let mut modifiers = Vec::new();
    let mut bonus = 0;

    for rule in &def.scaling {
        let held = lead_icons[rule.icon.index()];
        let contribution = (held * rule.permille_per_icon).min(rule.cap_permille);
        modifiers.push(Modifier {
            name: format!("scaling:{}", icon_slug(rule.icon)),
            permille: contribution,
        });
        bonus += contribution;
    }

    let mut breakpoints_met = Vec::new();
    let mut next_breakpoint = None;
    for breakpoint in &def.breakpoints {
        if lead_icons[breakpoint.icon.index()] >= breakpoint.at_least {
            modifiers.push(Modifier {
                name: format!("breakpoint:{}", breakpoint.id),
                permille: breakpoint.bonus_permille,
            });
            bonus += breakpoint.bonus_permille;
            breakpoints_met.push(breakpoint.id.clone());
        } else if next_breakpoint.is_none() {
            next_breakpoint = Some(breakpoint.clone());
        }
    }

    ReductionBonus {
        modifiers,
        bonus_permille: bonus,
        breakpoints_met,
        next_breakpoint,
    }
}

/// The repeat-curve multiplier (permille) for the `n`-th completion (0-based:
/// `n` completions have already happened, ever). Decommissioning never
/// decrements the count, so rebuild-toggling is never profitable.
pub fn cost_multiplier_permille(curve: CostCurve, completions_ever: u32) -> i64 {
    let n = i64::from(completions_ever);
    match curve {
        CostCurve::Flat => 1000,
        CostCurve::Linear { increment_permille } => 1000 + increment_permille * n,
        CostCurve::Exponential { growth_permille } => {
            let mut multiplier: i64 = 1000;
            for _ in 0..completions_ever {
                multiplier = multiplier * (1000 + growth_permille) / 1000;
            }
            multiplier
        }
        CostCurve::ScaleThenDeplete {
            discount_permille,
            floor_count,
            growth_permille,
        } => {
            let floor = i64::from(floor_count);
            if n <= floor {
                1000 - discount_permille * n
            } else {
                1000 - discount_permille * floor + growth_permille * (n - floor)
            }
        }
    }
}

fn icon_slug(icon: Icon) -> &'static str {
    match icon {
        Icon::Knowledge => "knowledge",
        Icon::Infrastructure => "infrastructure",
        Icon::Workforce => "workforce",
        Icon::Institutions => "institutions",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_price_the_documented_shapes() {
        assert_eq!(cost_multiplier_permille(CostCurve::Flat, 10), 1000);
        assert_eq!(
            cost_multiplier_permille(
                CostCurve::Linear {
                    increment_permille: 100
                },
                3
            ),
            1300
        );
        // Exponential compounds on the integer-truncated intermediate value.
        assert_eq!(
            cost_multiplier_permille(
                CostCurve::Exponential {
                    growth_permille: 100
                },
                2
            ),
            1210
        );
        let curve = CostCurve::ScaleThenDeplete {
            discount_permille: 40,
            floor_count: 5,
            growth_permille: 70,
        };
        assert_eq!(cost_multiplier_permille(curve, 0), 1000);
        assert_eq!(cost_multiplier_permille(curve, 5), 800);
        assert_eq!(cost_multiplier_permille(curve, 7), 940);
    }

    #[test]
    fn permille_application_rounds_down() {
        assert_eq!(apply_permille(600, 150), 690);
        assert_eq!(apply_permille(601, 150), 691);
        assert_eq!(apply_permille(1, 500), 1);
    }
}
