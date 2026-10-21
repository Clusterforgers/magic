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
///
/// Besides card names it knows printings (set and collector number) and the other names a printing
/// carries (flavor names, names printed on digital versions), which only ever resolve to the card
/// of their printing.
#[derive(Debug, Default)]
pub struct CardIndex {
    cards: Vec<IndexedCard>,
    seen: HashMap<OracleId, usize>,
    by_full_name: HashMap<String, Vec<usize>>,
    by_front_face: HashMap<String, Vec<usize>>,
    by_alias: HashMap<String, Vec<usize>>,
    by_alias_front: HashMap<String, Vec<usize>>,
    by_printing: HashMap<(String, String), Vec<usize>>,
    fuzzy_keys: Vec<FuzzyKey>,
}

/// Adds `slot` to the list under `key` unless it is already there.
fn add_slot<K: std::hash::Hash + Eq>(table: &mut HashMap<K, Vec<usize>>, key: K, slot: usize) {
    let slots = table.entry(key).or_default();
    if !slots.contains(&slot) {
        slots.push(slot);
    }
}

/// The front face of a normalized multi-face name.
fn front_face(key: &str) -> Option<&str> {
    key.split_once(FACE_SEPARATOR).map(|(front, _)| front)
}

/// A set code or collector number as the printing table compares it.
fn printing_part(text: &str) -> String {
    text.trim().to_lowercase()
}

fn lookup_in<'a>(cards: &'a [IndexedCard], slots: Option<&Vec<usize>>) -> Lookup<'a> {
    match slots.map(Vec::as_slice) {
        Some([only]) => Lookup::Found(&cards[*only]),
        Some([]) | None => Lookup::Missing,
        Some(many) => Lookup::Ambiguous(many.iter().map(|&s| &cards[s]).collect()),
    }
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
    ///
    /// Every record is a printing; a record from an all-printings file brings its flavor name and
    /// the name printed on its English digital version along.
    pub fn from_records<'a>(records: impl IntoIterator<Item = &'a CardRecord>) -> Self {
        let mut index = Self::new();
        for record in records.into_iter().filter(|r| r.layout.is_deck_card()) {
            index.insert_record(record);
        }
        index
    }

    /// Adds a record's card (once per oracle id), its printing and the other names of the printing.
    pub fn insert_record(&mut self, record: &CardRecord) {
        self.insert(record.oracle_id.clone(), &record.name);
        self.add_printing(&record.oracle_id, &record.set, &record.collector_number);
        if let Some(result) = &record.melds_into {
            // MTGJSON names a meld piece after both halves of the meld.
            self.add_alias(&record.oracle_id, &format!("{} // {result}", record.name));
        }
        if record.lang == "en" {
            for alias in [&record.flavor_name, &record.printed_name]
                .into_iter()
                .flatten()
            {
                self.add_alias(&record.oracle_id, alias);
            }
        }
    }

    /// Records that set `set` (any case) numbers a printing of the card `oracle_id` `number`; false when the card is unknown or a part is blank.
    pub fn add_printing(&mut self, oracle_id: &OracleId, set: &str, number: &str) -> bool {
        let Some(&slot) = self.seen.get(oracle_id) else {
            return false;
        };
        let (set, number) = (printing_part(set), printing_part(number));
        if set.is_empty() || number.is_empty() {
            return false;
        }
        add_slot(&mut self.by_printing, (set, number), slot);
        true
    }

    /// Adds another name of a known card, such as a flavor name; false when the card is unknown, the name blank or already the card's own.
    ///
    /// Such names are only consulted after card names, so one can never take a card name away from its card.
    pub fn add_alias(&mut self, oracle_id: &OracleId, name: &str) -> bool {
        let Some(&slot) = self.seen.get(oracle_id) else {
            return false;
        };
        let key = normalize_name(name);
        let own = normalize_name(&self.cards[slot].name);
        let known = self.by_alias.get(&key).is_some_and(|s| s.contains(&slot));
        if key.is_empty() || key == own || known {
            return false;
        }
        if let Some(front) = front_face(&key) {
            add_slot(&mut self.by_alias_front, front.to_owned(), slot);
            self.push_fuzzy_key(front, slot);
        }
        self.push_fuzzy_key(&key, slot);
        add_slot(&mut self.by_alias, key, slot);
        true
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

        if let Some(front) = front_face(&full) {
            add_slot(&mut self.by_front_face, front.to_owned(), slot);
            for face in full.split(FACE_SEPARATOR) {
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
        self.first_hit(&[&self.by_full_name, &self.by_front_face], &key)
    }

    /// Finds a card by a name one of its printings carries (flavor name, name printed on a digital version), in full or by its front face.
    pub fn find_alias(&self, name: &str) -> Lookup<'_> {
        let key = normalize_name(name);
        self.first_hit(&[&self.by_alias, &self.by_alias_front], &key)
    }

    /// Finds the card of the printing numbered `number` in the set `set`, ignoring case.
    pub fn find_printing(&self, set: &str, number: &str) -> Lookup<'_> {
        let key = (printing_part(set), printing_part(number));
        lookup_in(&self.cards, self.by_printing.get(&key))
    }

    fn first_hit(&self, tables: &[&HashMap<String, Vec<usize>>], key: &str) -> Lookup<'_> {
        tables
            .iter()
            .find_map(|table| table.get(key))
            .map_or(Lookup::Missing, |slots| lookup_in(&self.cards, Some(slots)))
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

    fn id(n: usize) -> OracleId {
        OracleId(format!("oracle-{n}"))
    }

    fn named(lookup: Lookup<'_>) -> Vec<String> {
        match lookup {
            Lookup::Found(card) => vec![card.name.clone()],
            Lookup::Ambiguous(cards) => cards.iter().map(|c| c.name.clone()).collect(),
            Lookup::Missing => Vec::new(),
        }
    }

    /// `pool()` plus printings and other names (oracle-0 is Lightning Bolt, oracle-2 Fire // Ice, oracle-5 Æther Vial, oracle-9 Jace).
    fn printed_pool() -> CardIndex {
        let mut index = pool();
        for (card, set, number) in [
            (0, "M10", "146"),
            (0, "sld", "1100"),
            (2, "APC", "128"),
            (5, "ima", "1A★"),
        ] {
            assert!(index.add_printing(&id(card), set, number));
        }
        for (card, alias) in [
            (0, "Bolt of the Kitten"),
            (2, "Frost // Flame"),
            (3, "Bug Catcher // Big Bug"),
            (9, "Sol Ring"),
        ] {
            assert!(index.add_alias(&id(card), alias));
        }
        index
    }

    #[rstest]
    #[case::exact("M10", "146", &["Lightning Bolt"])]
    #[case::lowercase_set("m10", "146", &["Lightning Bolt"])]
    #[case::padded("  M10 ", " 146", &["Lightning Bolt"])]
    #[case::second_printing("SLD", "1100", &["Lightning Bolt"])]
    #[case::split_card("apc", "128", &["Fire // Ice"])]
    #[case::collector_number_with_symbols("IMA", "1a★", &["Æther Vial"])]
    #[case::other_number("M10", "147", &[])]
    #[case::other_set("M11", "146", &[])]
    #[case::number_is_not_a_prefix("M10", "14", &[])]
    #[case::blank_set("", "146", &[])]
    #[case::blank_number("M10", "", &[])]
    fn finds_the_card_of_a_printing(
        #[case] set: &str,
        #[case] number: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(named(printed_pool().find_printing(set, number)), expected);
    }

    #[test]
    fn printings_of_unknown_cards_and_blank_parts_are_refused() {
        let mut index = pool();
        assert!(!index.add_printing(&OracleId("nobody".into()), "M10", "1"));
        assert!(!index.add_printing(&id(0), " ", "1"));
        assert!(!index.add_printing(&id(0), "M10", ""));
        assert!(index.add_printing(&id(0), "M10", "146"));
        assert!(index.add_printing(&id(0), "M10", "146"), "again is fine");
        assert_eq!(named(index.find_printing("M10", "146")), ["Lightning Bolt"]);
    }

    #[test]
    fn two_cards_on_one_printing_are_ambiguous() {
        let mut index = pool();
        index.add_printing(&id(0), "XYZ", "1");
        index.add_printing(&id(1), "XYZ", "1");
        index.add_printing(&id(1), "XYZ", "1");
        let mut found = named(index.find_printing("xyz", "1"));
        found.sort();
        assert_eq!(found, ["Lightning Bolt", "Lightning Helix"]);
    }

    #[rstest]
    #[case::flavor("Bolt of the Kitten", &["Lightning Bolt"])]
    #[case::lowercase("bolt of the kitten", &["Lightning Bolt"])]
    #[case::split_flavor_in_full("Frost // Flame", &["Fire // Ice"])]
    #[case::split_flavor_single_slash("Frost/Flame", &["Fire // Ice"])]
    #[case::split_flavor_front_only("Frost", &["Fire // Ice"])]
    #[case::transform_flavor_front_only("Bug Catcher", &["Delver of Secrets // Insectile Aberration"])]
    #[case::name_of_another_card_too("Sol Ring", &["Jace, the Mind Sculptor"])]
    #[case::second_half_only("Flame", &[])]
    #[case::unknown("Black Lotus", &[])]
    fn finds_the_card_by_another_name_of_a_printing(
        #[case] query: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(named(printed_pool().find_alias(query)), expected);
    }

    #[test]
    fn other_names_never_stand_in_for_card_names() {
        let index = printed_pool();
        assert_eq!(named(index.find("Sol Ring")), ["Sol Ring"]);
        assert_eq!(index.find("Bolt of the Kitten"), Lookup::Missing);
        assert_eq!(index.find("Frost"), Lookup::Missing);
    }

    #[test]
    fn other_names_of_unknown_cards_blank_or_already_known_are_refused() {
        let mut index = pool();
        assert!(!index.add_alias(&OracleId("nobody".into()), "Frost"));
        assert!(!index.add_alias(&id(0), "  "));
        assert!(!index.add_alias(&id(0), "lightning BOLT"), "its own name");
        assert!(index.add_alias(&id(0), "Bolt of the Kitten"));
        assert!(!index.add_alias(&id(0), "bolt of the kitten"), "known");
        let kitten = index.suggest("Bolt of the Kitten", 10);
        assert_eq!(kitten.len(), 1, "{kitten:?}");
    }

    #[test]
    fn one_name_on_two_cards_is_ambiguous() {
        let mut index = pool();
        index.add_alias(&id(0), "Shared");
        index.add_alias(&id(1), "Shared");
        let mut found = named(index.find_alias("shared"));
        found.sort();
        assert_eq!(found, ["Lightning Bolt", "Lightning Helix"]);
    }

    #[test]
    fn suggestions_include_other_names_under_the_card_name() {
        let suggestions = printed_pool().suggest("Bolt of the Kiten", 3);
        assert_eq!(suggestions[0].name, "Lightning Bolt");
        assert_eq!(suggestions[0].distance, 1);
    }

    #[test]
    fn building_from_records_takes_printings_and_other_names_of_english_printings() {
        use crate::scryfall::cards;
        let json = r#"[
          {"id":"1","oracle_id":"o1","name":"Fellwar Stone","layout":"normal","set":"c21","collector_number":"259"},
          {"id":"2","oracle_id":"o1","name":"Fellwar Stone","layout":"normal","set":"sld","collector_number":"1040",
           "flavor_name":"The Ball"},
          {"id":"3","oracle_id":"o2","name":"Peter Parker's Camera","layout":"normal","set":"om1","collector_number":"171",
           "printed_name":"Phenomena Recorder","digital":true},
          {"id":"4","oracle_id":"o2","name":"Peter Parker's Camera","layout":"normal","set":"spm","collector_number":"171",
           "lang":"de","printed_name":"Peter Parkers Kamera"},
          {"id":"5","oracle_id":"o3","name":"Goblin","layout":"token","set":"tc21","collector_number":"1",
           "flavor_name":"Gobbo"},
          {"id":"6","oracle_id":"o4","name":"Gisela, the Broken Blade","layout":"meld","set":"emn","collector_number":"28",
           "all_parts":[{"component":"meld_result","name":"Brisela, Voice of Nightmares"},
                        {"component":"meld_part","name":"Gisela, the Broken Blade"}]}
        ]"#;
        let records: Vec<_> = cards(json.as_bytes()).map(Result::unwrap).collect();
        let index = CardIndex::from_records(&records);
        assert_eq!(index.len(), 3);
        let meld = "Gisela, the Broken Blade // Brisela, Voice of Nightmares";
        assert_eq!(named(index.find_alias(meld)), ["Gisela, the Broken Blade"]);
        assert_eq!(index.find(meld), Lookup::Missing);
        assert_eq!(named(index.find_printing("C21", "259")), ["Fellwar Stone"]);
        assert_eq!(named(index.find_printing("SLD", "1040")), ["Fellwar Stone"]);
        assert_eq!(
            named(index.find_printing("om1", "171")),
            ["Peter Parker's Camera"]
        );
        assert_eq!(named(index.find_alias("The Ball")), ["Fellwar Stone"]);
        assert_eq!(
            named(index.find_alias("Phenomena Recorder")),
            ["Peter Parker's Camera"]
        );
        assert_eq!(index.find_alias("Peter Parkers Kamera"), Lookup::Missing);
        assert_eq!(index.find_alias("Gobbo"), Lookup::Missing);
        assert_eq!(index.find_printing("tc21", "1"), Lookup::Missing);
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
