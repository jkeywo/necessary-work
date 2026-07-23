//! World structure: the three continents, three sectors per continent, and four
//! capacity-icon families. Differences between continents are authored starting
//! systems and bottlenecks, never innate regional traits or development
//! rankings. These enums live in the content crate so authored data and the
//! simulation share one vocabulary; `nw-simulation::world` re-exports them.

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

    /// Stable array index for state tables.
    pub fn index(self) -> usize {
        match self {
            Continent::Europe => 0,
            Continent::NorthAmerica => 1,
            Continent::MajorityWorld => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Continent::Europe => "Europe",
            Continent::NorthAmerica => "North America",
            Continent::MajorityWorld => "Majority World",
        }
    }
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

    /// Stable array index for state tables.
    pub fn index(self) -> usize {
        match self {
            Sector::Power => 0,
            Sector::TransportAndBuildings => 1,
            Sector::IndustryAndLand => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Sector::Power => "Power",
            Sector::TransportAndBuildings => "Transport & Buildings",
            Sector::IndustryAndLand => "Industry & Land",
        }
    }
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

    /// Stable array index for state tables.
    pub fn index(self) -> usize {
        match self {
            Icon::Knowledge => 0,
            Icon::Infrastructure => 1,
            Icon::Workforce => 2,
            Icon::Institutions => 3,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Icon::Knowledge => "Knowledge",
            Icon::Infrastructure => "Infrastructure",
            Icon::Workforce => "Workforce",
            Icon::Institutions => "Institutions",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prototype_tracks_nine_sector_rates() {
        assert_eq!(Continent::ALL.len() * Sector::ALL.len(), 9);
    }

    #[test]
    fn indices_are_stable_and_dense() {
        for (position, continent) in Continent::ALL.iter().enumerate() {
            assert_eq!(continent.index(), position);
        }
        for (position, sector) in Sector::ALL.iter().enumerate() {
            assert_eq!(sector.index(), position);
        }
        for (position, icon) in Icon::ALL.iter().enumerate() {
            assert_eq!(icon.index(), position);
        }
    }
}
