//! Goal: learn, from a deck's own data, the names its texts use instead of oracle names (flavor names, front faces) so the comparison stays strict about everything else.

use std::collections::BTreeMap;

use mtg_import::decklist::{DeckFormat, parse_as};

use super::truth::Deck;

/// Writes the faces of a multi-face card the oracle way (`A // B`); Moxfield says `A / B`, Arena `A /// B`, MTGO `A/B`.
pub fn canonical(name: &str) -> String {
    let faces: Vec<&str> = name
        .split('/')
        .map(str::trim)
        .filter(|face| !face.is_empty())
        .collect();
    faces.join(" // ")
}

/// Characters that only a parser leaving markers in a name could produce.
pub const MARKERS: [char; 7] = ['#', '*', '[', ']', '{', '}', '^'];

/// A flavor name shares nothing with the oracle name; a polluted name always contains it or a marker.
fn is_flavor_name(name: &str, oracle: &str) -> bool {
    !name.contains(oracle) && !oracle.contains(name) && !name.contains(MARKERS)
}

/// Names a deck's texts may use instead of the oracle name.
#[derive(Debug, Default)]
pub struct Aliases {
    /// Moxfield flavor names: where `full` names a printing differently from the site's truth.
    flavor: BTreeMap<String, String>,
    /// Front face to full name of every multi-face card, for texts that write the front face alone.
    faces: BTreeMap<String, String>,
}

impl Aliases {
    /// Learns the aliases of `deck`: flavor names from `full` joined with the truth on set and collector number.
    pub fn of(deck: &Deck) -> Aliases {
        let boards = &deck.truth.boards;
        let rows = [
            &boards.commanders,
            &boards.signature_spells,
            &boards.mainboard,
            &boards.sideboard,
            &boards.maybeboard,
            &boards.stickers,
        ]
        .into_iter()
        .flatten();
        let mut by_printing = BTreeMap::new();
        let mut faces = BTreeMap::new();
        for row in rows {
            let name = canonical(&row.name);
            if let Some((front, _)) = name.split_once(" // ") {
                faces.insert(front.to_owned(), name.clone());
            }
            by_printing.insert((row.set.clone(), row.cn.clone()), name);
        }
        let flavor = deck
            .try_text("full")
            .map(|text| parse_as(DeckFormat::Moxfield, &text).entries)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| {
                let printing = (entry.set?.to_lowercase(), entry.collector_number?);
                let oracle = by_printing.get(&printing)?;
                let name = canonical(&entry.name);
                is_flavor_name(&name, oracle).then(|| (name, oracle.clone()))
            })
            .collect();
        Aliases { flavor, faces }
    }

    /// The oracle name behind `name` as a text wrote it, undoing flavor names and (when `front_names`) front-face-only names.
    pub fn oracle_name(&self, name: &str, front_names: bool) -> String {
        let name = canonical(name);
        let name = self.flavor.get(&name).cloned().unwrap_or(name);
        match self.faces.get(&name) {
            Some(full) if front_names => full.clone(),
            _ => name,
        }
    }

    /// The learned flavor names, flavor name to oracle name.
    pub fn flavor_names(&self) -> &BTreeMap<String, String> {
        &self.flavor
    }
}
