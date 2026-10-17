//! In-memory card-name index supporting exact (normalized) lookup and fuzzy suggestions.

use std::collections::HashMap;

use super::distance::bounded_distance;
use super::normalize::normalize_name;
use crate::ids::OracleId;
use crate::scryfall::CardRecord;

/// Separator between the faces of a multi-faced card in a normalized name.
const FACE_SEPARATOR: &str = " // ";

/// Shortest query for which a prefix of a card name counts as a suggestion.
const MIN_PREFIX_CHARS: usize = 4;

/// A card name proposed (or matched) for a query, with its edit distance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Card the name belongs to.
    pub oracle_id: OracleId,
    /// Canonical full card name.
    pub name: String,
    /// Edit distance from the query to the closest matching name or face name.
    pub distance: usize,
}

/// A card known to the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedCard {
    /// Card identity.
    pub oracle_id: OracleId,
    /// Canonical full name, `A // B` for multi-faced cards.
    pub name: String,
}

/// Outcome of an exact lookup.
#[derive(Debug, PartialEq, Eq)]
pub enum Lookup<'a> {
    /// Exactly one card has this name.
    Found(&'a IndexedCard),
    /// Several different cards share this name.
    Ambiguous(Vec<&'a IndexedCard>),
    /// No card has this name.
    Missing,
}

/// A normalized name (full or single face) that fuzzy search compares against.
#[derive(Debug)]
struct FuzzyKey {
    chars: Vec<char>,
    card: usize,
}

/// Name index over a card pool; each oracle id is stored once.
#[derive(Debug, Default)]
pub struct CardIndex {
    cards: Vec<IndexedCard>,
    seen: HashMap<OracleId, usize>,
    by_full_name: HashMap<String, Vec<usize>>,
    by_front_face: HashMap<String, Vec<usize>>,
    fuzzy_keys: Vec<FuzzyKey>,
}

/// Largest edit distance still considered a plausible typo for a query of `len` characters.
fn max_distance(len: usize) -> usize {
    (1 + len / 5).min(4)
}

impl CardIndex {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds an index from card records, skipping tokens and other non-deck objects.
    pub fn from_records<'a>(records: impl IntoIterator<Item = &'a CardRecord>) -> Self {
        let mut index = Self::new();
        for record in records.into_iter().filter(|r| r.layout.is_deck_card()) {
            index.insert(record.oracle_id.clone(), &record.name);
        }
        index
    }

    /// Adds a card; later insertions of an already known oracle id and unusable names are ignored.
    pub fn insert(&mut self, oracle_id: OracleId, name: &str) {
        let full = normalize_name(name);
        if full.is_empty() || self.seen.contains_key(&oracle_id) {
            return;
        }
        let slot = self.cards.len();
        self.seen.insert(oracle_id.clone(), slot);
        self.cards.push(IndexedCard {
            oracle_id,
            name: name.trim().to_owned(),
        });

        let faces: Vec<&str> = full.split(FACE_SEPARATOR).collect();
        if faces.len() > 1 {
            if let Some(front) = faces.first() {
                self.by_front_face
                    .entry((*front).to_owned())
                    .or_default()
                    .push(slot);
            }
            for face in &faces {
                self.push_fuzzy_key(face, slot);
            }
        }
        self.push_fuzzy_key(&full, slot);
        self.by_full_name.entry(full).or_default().push(slot);
    }

    fn push_fuzzy_key(&mut self, key: &str, card: usize) {
        self.fuzzy_keys.push(FuzzyKey {
            chars: key.chars().collect(),
            card,
        });
    }

    /// Number of distinct cards.
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Whether the index holds no cards.
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Finds a card by full name, then by front-face name, after normalization.
    pub fn find(&self, name: &str) -> Lookup<'_> {
        let key = normalize_name(name);
        for table in [&self.by_full_name, &self.by_front_face] {
            if let Some(slots) = table.get(&key) {
                return match slots.as_slice() {
                    [only] => Lookup::Found(&self.cards[*only]),
                    many => Lookup::Ambiguous(many.iter().map(|&s| &self.cards[s]).collect()),
                };
            }
        }
        Lookup::Missing
    }

    /// Proposes up to `limit` cards whose full or face names are close to `name`, best first.
    pub fn suggest(&self, name: &str, limit: usize) -> Vec<Suggestion> {
        let query: Vec<char> = normalize_name(name).chars().collect();
        if query.is_empty() {
            return Vec::new();
        }
        let max = max_distance(query.len());
        let prefix_allowed = query.len() >= MIN_PREFIX_CHARS;
        let mut best: HashMap<usize, usize> = HashMap::new();
        for key in &self.fuzzy_keys {
            let distance = bounded_distance(&query, &key.chars, max).or_else(|| {
                (prefix_allowed && key.chars.starts_with(&query))
                    .then(|| key.chars.len() - query.len())
            });
            if let Some(distance) = distance {
                best.entry(key.card)
                    .and_modify(|d| *d = (*d).min(distance))
                    .or_insert(distance);
            }
        }
        let mut suggestions: Vec<Suggestion> = best
            .into_iter()
            .map(|(slot, distance)| Suggestion {
                oracle_id: self.cards[slot].oracle_id.clone(),
                name: self.cards[slot].name.clone(),
                distance,
            })
            .collect();
        suggestions.sort_by(|a, b| {
            a.distance
                .cmp(&b.distance)
                .then_with(|| a.name.cmp(&b.name))
        });
        suggestions.truncate(limit);
        suggestions
    }
}

#[cfg(test)]
mod tests {
    //! Goal: exact lookup is forgiving only in ways that cannot confuse two cards, and fuzzy search proposes near names without ever choosing one.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn index(names: &[&str]) -> CardIndex {
        let mut index = CardIndex::new();
        for (n, name) in names.iter().enumerate() {
            index.insert(OracleId(format!("oracle-{n}")), name);
        }
        index
    }

    fn pool() -> CardIndex {
        index(&[
            "Lightning Bolt",
            "Lightning Helix",
            "Fire // Ice",
            "Delver of Secrets // Insectile Aberration",
            "Bonecrusher Giant // Stomp",
            "Æther Vial",
            "Lim-Dûl's Vault",
            "Atraxa, Praetors' Voice",
            "Sol Ring",
            "Jace, the Mind Sculptor",
            "Island",
        ])
    }

    fn found(index: &CardIndex, query: &str) -> Option<String> {
        match index.find(query) {
            Lookup::Found(card) => Some(card.name.clone()),
            _ => None,
        }
    }

    #[rstest]
    #[case::exact("Lightning Bolt", "Lightning Bolt")]
    #[case::lowercase("lightning bolt", "Lightning Bolt")]
    #[case::shouting("LIGHTNING BOLT", "Lightning Bolt")]
    #[case::padded("  Lightning   Bolt ", "Lightning Bolt")]
    #[case::split_full("Fire // Ice", "Fire // Ice")]
    #[case::split_single_slash("Fire/Ice", "Fire // Ice")]
    #[case::split_front_only("Fire", "Fire // Ice")]
    #[case::transform_front_only("Delver of Secrets", "Delver of Secrets // Insectile Aberration")]
    #[case::transform_full(
        "delver of secrets // insectile aberration",
        "Delver of Secrets // Insectile Aberration"
    )]
    #[case::adventure_front_only("Bonecrusher Giant", "Bonecrusher Giant // Stomp")]
    #[case::ligature_to_ascii("Aether Vial", "Æther Vial")]
    #[case::ascii_to_ligature_case("æther vial", "Æther Vial")]
    #[case::accent_dropped("Lim-Dul's Vault", "Lim-Dûl's Vault")]
    #[case::curly_apostrophe("Lim-Dûl’s Vault", "Lim-Dûl's Vault")]
    #[case::curly_apostrophe_atraxa("Atraxa, Praetors’ Voice", "Atraxa, Praetors' Voice")]
    fn finds_equivalent_spellings(#[case] query: &str, #[case] expected: &str) {
        assert_eq!(found(&pool(), query).as_deref(), Some(expected));
    }

    #[rstest]
    #[case::back_face_only("Stomp")]
    #[case::second_half_only("Ice")]
    #[case::typo("Lightnin Bolt")]
    #[case::missing_comma("Jace the Mind Sculptor")]
    #[case::partial("Lightning")]
    #[case::empty("")]
    #[case::spaces("   ")]
    #[case::unrelated("Black Lotus")]
    #[case::reversed_halves("Ice // Fire")]
    #[case::dangling_slash("Fire //")]
    fn does_not_guess(#[case] query: &str) {
        assert_eq!(pool().find(query), Lookup::Missing);
    }

    #[test]
    fn same_normalized_name_on_two_cards_is_ambiguous() {
        let index = index(&["Étude", "Etude", "Other"]);
        match index.find("etude") {
            Lookup::Ambiguous(cards) => {
                let mut names: Vec<_> = cards.iter().map(|c| c.name.as_str()).collect();
                names.sort_unstable();
                assert_eq!(names, ["Etude", "Étude"]);
            }
            other => panic!("expected ambiguity, got {other:?}"),
        }
    }

    #[test]
    fn full_name_beats_front_face_of_another_card() {
        let index = index(&["Fire", "Fire // Ice"]);
        assert_eq!(found(&index, "Fire").as_deref(), Some("Fire"));
        assert_eq!(found(&index, "Fire // Ice").as_deref(), Some("Fire // Ice"));
    }

    #[test]
    fn two_cards_sharing_a_front_face_are_ambiguous_by_that_face() {
        let index = index(&["Twin // First", "Twin // Second"]);
        assert!(matches!(index.find("Twin"), Lookup::Ambiguous(c) if c.len() == 2));
        assert!(matches!(index.find("Twin // First"), Lookup::Found(_)));
    }

    #[test]
    fn repeated_oracle_ids_and_blank_names_are_ignored() {
        let mut index = CardIndex::new();
        index.insert(OracleId("a".into()), "First Name");
        index.insert(OracleId("a".into()), "Second Name");
        index.insert(OracleId("b".into()), "   ");
        index.insert(OracleId("c".into()), "\u{200b}");
        assert_eq!(index.len(), 1);
        assert!(!index.is_empty());
        assert_eq!(found(&index, "First Name").as_deref(), Some("First Name"));
        assert_eq!(index.find("Second Name"), Lookup::Missing);
        assert!(CardIndex::new().is_empty());
    }

    #[rstest]
    #[case::dropped_letter("Lightnin Bolt", "Lightning Bolt")]
    #[case::extra_letter("Lightning Boltt", "Lightning Bolt")]
    #[case::swapped_letters("Lightnign Bolt", "Lightning Bolt")]
    #[case::wrong_letter("Lightning Bolf", "Lightning Bolt")]
    #[case::missing_space("LightningBolt", "Lightning Bolt")]
    #[case::missing_comma("Jace the Mind Sculptor", "Jace, the Mind Sculptor")]
    #[case::missing_apostrophe("Atraxa, Praetors Voice", "Atraxa, Praetors' Voice")]
    #[case::second_half_of_split("Ice", "Fire // Ice")]
    #[case::back_face_of_adventure("Stomp", "Bonecrusher Giant // Stomp")]
    #[case::back_face_typo("Insectile Aberation", "Delver of Secrets // Insectile Aberration")]
    #[case::truncated_name("Lightning", "Lightning Bolt")]
    #[case::sol_ring_swap("Sol Rign", "Sol Ring")]
    fn suggests_the_intended_card_first(#[case] query: &str, #[case] expected: &str) {
        let suggestions = pool().suggest(query, 5);
        assert_eq!(
            suggestions.first().map(|s| s.name.as_str()),
            Some(expected),
            "{suggestions:?}"
        );
    }

    #[test]
    fn suggestions_are_sorted_distinct_and_limited() {
        let suggestions = pool().suggest("Lightning", 1);
        assert_eq!(suggestions.len(), 1);
        let all = pool().suggest("Lightning Bolx", 10);
        assert!(
            all.windows(2)
                .all(|w| (w[0].distance, &w[0].name) <= (w[1].distance, &w[1].name))
        );
        let mut ids: Vec<_> = all.iter().map(|s| &s.oracle_id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        assert_eq!(all[0].name, "Lightning Bolt");
        assert_eq!(all[0].distance, 1);
    }

    #[rstest]
    #[case::unrelated("Black Lotus")]
    #[case::empty("")]
    #[case::whitespace("  ")]
    #[case::far_typo("Lxxxxxxxxg Bxxx")]
    #[case::short_noise("Zz")]
    fn far_names_get_no_suggestions(#[case] query: &str) {
        assert_eq!(pool().suggest(query, 5), Vec::new());
    }

    #[test]
    fn short_prefixes_are_not_suggested() {
        assert_eq!(pool().suggest("Lig", 5), Vec::new());
        assert_ne!(pool().suggest("Ligh", 5), Vec::new());
    }

    #[test]
    fn zero_limit_gives_nothing() {
        assert_eq!(pool().suggest("Lightnin Bolt", 0), Vec::new());
    }

    #[test]
    fn building_from_records_skips_tokens_and_art_series() {
        use crate::scryfall::cards;
        let json = r#"[
          {"id":"1","oracle_id":"o1","name":"Goblin","layout":"token"},
          {"id":"2","oracle_id":"o2","name":"Lightning Bolt","layout":"normal"},
          {"id":"3","oracle_id":"o3","name":"Lightning Bolt // Lightning Bolt","layout":"art_series",
           "card_faces":[{"name":"Lightning Bolt"},{"name":"Lightning Bolt"}]},
          {"id":"4","oracle_id":"o2","name":"Lightning Bolt","layout":"normal"},
          {"id":"5","oracle_id":"o5","name":"Emblem","layout":"emblem"}
        ]"#;
        let records: Vec<_> = cards(json.as_bytes()).map(Result::unwrap).collect();
        let index = CardIndex::from_records(&records);
        assert_eq!(index.len(), 1);
        assert_eq!(
            found(&index, "Lightning Bolt").as_deref(),
            Some("Lightning Bolt")
        );
        assert_eq!(index.find("Goblin"), Lookup::Missing);
    }

    fn name_strategy() -> impl Strategy<Value = String> {
        "[a-z]{3,8}( [a-z]{3,8}){0,2}"
    }

    proptest! {
        #[test]
        fn every_inserted_name_finds_itself(names in prop::collection::btree_set(name_strategy(), 1..30)) {
            let names: Vec<String> = names.into_iter().collect();
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let index = index(&refs);
            for name in &names {
                let exact = found(&index, name);
                let shouted = found(&index, &name.to_uppercase());
                prop_assert_eq!(exact.as_deref(), Some(name.as_str()));
                prop_assert_eq!(shouted.as_deref(), Some(name.as_str()));
            }
        }

        #[test]
        fn single_typos_are_always_suggested_and_never_applied(
            names in prop::collection::btree_set("[a-z]{6,10}", 2..25),
            pick in any::<prop::sample::Index>(),
            position in any::<prop::sample::Index>(),
            replacement in prop::sample::select(vec!['0', '1', '2']),
        ) {
            let names: Vec<String> = names.into_iter().collect();
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let index = index(&refs);
            let target = pick.get(&names);
            let mut typo: Vec<char> = target.chars().collect();
            let slot = position.index(typo.len());
            typo[slot] = replacement;
            let typo: String = typo.into_iter().collect();
            prop_assert_eq!(index.find(&typo), Lookup::Missing);
            let suggested = index.suggest(&typo, usize::MAX);
            prop_assert!(suggested.iter().any(|s| &s.name == target && s.distance == 1));
        }
    }
}
