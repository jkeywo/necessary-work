//! World structure: the three continents, three sectors per continent, and four
//! capacity-icon families. Differences between continents are authored starting
//! systems and bottlenecks, never innate regional traits or development
//! rankings.

use serde::{Deserialize, Serialize};

/// The three continental regions. Europe hosts the CFA secretariat for
/// contingent historical/treaty reasons only: it grants no efficiency bonus.
/// "Majority World" is a provisional prototype label requiring review before
/// public release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Continent {
    Europe,
    NorthAmerica,
    /// Provisional label — see `pasm/spec/core/framing.yaml`.
    MajorityWorld,
}

impl Continent {
    /// All continents, in a stable order.
    pub const ALL: [Continent; 3] = [
        Continent::Europe,
        Continent::NorthAmerica,
        Continent::MajorityWorld,
    ];
}

/// The three emission sectors tracked per continent. Grouped for prototype
/// simplicity; project context explains the grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sector {
    Power,
    TransportAndBuildings,
    IndustryAndLand,
}

impl Sector {
    /// All sectors, in a stable order.
    pub const ALL: [Sector; 3] = [
        Sector::Power,
        Sector::TransportAndBuildings,
        Sector::IndustryAndLand,
    ];
}

/// Capacity icons: installed capacities tracked separately per continent, not
/// spendable resources and never pooled globally. They represent installed
/// systems, not cultural or governance superiority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Icon {
    Knowledge,
    Infrastructure,
    Workforce,
    Institutions,
}

impl Icon {
    /// All icon families, in a stable order.
    pub const ALL: [Icon; 4] = [
        Icon::Knowledge,
        Icon::Infrastructure,
        Icon::Workforce,
        Icon::Institutions,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prototype_tracks_nine_sector_rates() {
        assert_eq!(Continent::ALL.len() * Sector::ALL.len(), 9);
    }

    #[test]
    fn four_icon_families_per_continent() {
        assert_eq!(Icon::ALL.len(), 4);
    }
}
