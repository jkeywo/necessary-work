//! World structure, re-exported from the content crate so authored data and
//! the simulation share one vocabulary. Differences between continents are
//! authored starting systems and bottlenecks, never innate regional traits.

pub use nw_content::world::{Continent, Icon, Sector};

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
