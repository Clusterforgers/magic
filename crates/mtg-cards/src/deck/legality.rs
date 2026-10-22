//! Legality: whether a deck follows the rules of its format, reported as a list of issues apart from
//! how far the game plays the cards. The rules are the ones of `docs/deck-check.md`: Commander, the
//! 60-card constructed formats and freeform.

use std::collections::BTreeMap;

use mtg_core::color::{Color, ColorSet};
use mtg_core::house::HouseRules;
use serde::{Deserialize, Serialize};

use super::{CardFacts, DeckZone, Legality, name_of};

/// Cards in a Commander deck, its commanders included.
const COMMANDER_SIZE: u32 = 100;
/// Fewest cards in the main deck of a constructed format.
const CONSTRUCTED_MIN: u32 = 60;
/// Most cards in the sideboard of a constructed format.
const SIDEBOARD_MAX: u32 = 15;
/// Copies of a card a constructed deck may hold, sideboard included.
const CONSTRUCTED_COPIES: u32 = 4;

/// The 60-card formats the check knows, by their Scryfall legality key.
const CONSTRUCTED: [&str; 12] = [
    "standard",
    "pioneer",
    "modern",
    "legacy",
    "vintage",
    "pauper",
    "historic",
    "timeless",
    "alchemy",
    "penny",
    "premodern",
    "oldschool",
];

/// Every format a deck can be checked in, by its lower-case key.
pub const SUPPORTED_FORMATS: [&str; 14] = [
    "commander",
    "freeform",
    "standard",
    "pioneer",
    "modern",
    "legacy",
    "vintage",
    "pauper",
    "historic",
    "timeless",
    "alchemy",
    "penny",
    "premodern",
    "oldschool",
];

/// Whether the check has rules for `format`.
pub fn is_supported_format(format: &str) -> bool {
    SUPPORTED_FORMATS.contains(&format)
}

/// The family of rules a format belongs to.
enum Rules {
    Freeform,
    Commander,
    Constructed,
}

fn rules_of(format: &str) -> Option<Rules> {
    match format {
        "freeform" => Some(Rules::Freeform),
        "commander" => Some(Rules::Commander),
        _ if CONSTRUCTED.contains(&format) => Some(Rules::Constructed),
        _ => None,
    }
}

/// What a legality issue is about; the wire spells it in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum IssueCode {
    /// The deck has no cards.
    Empty,
    /// The check has no rules for the format.
    UnknownFormat,
    /// A card is not in the card database.
    UnknownCard,
    /// Too few or too many cards for the format.
    DeckSize,
    /// More cards in the sideboard than the format allows.
    SideboardSize,
    /// More than one companion.
    CompanionCount,
    /// Cards in the command zone of a format that has none.
    NoCommanderZone,
    /// No commander, or more than two.
    CommanderCount,
    /// A card that cannot be a commander.
    CommanderEligibility,
    /// Two commanders that cannot be commanders together.
    CommanderPair,
    /// More copies of a card than the format allows.
    Copies,
    /// A card with a color outside the commander's color identity.
    ColorIdentity,
    /// A card the format bans.
    Banned,
    /// A card that is not in the format's card pool.
    NotLegal,
}

impl IssueCode {
    /// Whether the deck rules of some table can lift this issue: the size, the copies, the colour identity and the
    /// pair of commanders are rules a table may change, the others hold at every table.
    pub const fn liftable(self) -> bool {
        matches!(
            self,
            IssueCode::DeckSize
                | IssueCode::Copies
                | IssueCode::ColorIdentity
                | IssueCode::CommanderPair
        )
    }

    /// Whether the deck rules of a table with `house` rules lift this issue: the format's own check no longer applies,
    /// and the engine's `validate_seat` under the table's rules judges the rule instead.
    pub fn lifted_by(self, house: &HouseRules) -> bool {
        let deck = &house.deck;
        match self {
            IssueCode::DeckSize => deck.deck_size.is_some(),
            IssueCode::Copies => deck.singleton == Some(false),
            IssueCode::ColorIdentity => deck.colour_identity == Some(false),
            IssueCode::CommanderPair => house.commanders_per_seat == Some(2),
            _ => false,
        }
    }
}

/// One way a deck breaks the rules of its format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Issue {
    /// What kind of problem it is.
    pub code: IssueCode,
    /// The problem in plain words.
    pub message: String,
    /// The zone it sits in, when it is about a zone or a card.
    pub zone: Option<DeckZone>,
    /// The card it is about, when it is about one.
    pub oracle_id: Option<String>,
    /// That card's name.
    pub name: Option<String>,
}

impl Issue {
    fn deck(code: IssueCode, message: impl Into<String>) -> Issue {
        Issue {
            code,
            message: message.into(),
            zone: None,
            oracle_id: None,
            name: None,
        }
    }

    fn zone(code: IssueCode, zone: DeckZone, message: impl Into<String>) -> Issue {
        Issue {
            zone: Some(zone),
            ..Issue::deck(code, message)
        }
    }

    fn card(
        code: IssueCode,
        zone: DeckZone,
        id: &str,
        facts: Option<&CardFacts>,
        message: impl Into<String>,
    ) -> Issue {
        Issue {
            zone: Some(zone),
            oracle_id: Some(id.to_owned()),
            name: Some(name_of(facts, id).to_owned()),
            ..Issue::deck(code, message)
        }
    }
}

type Deck = BTreeMap<(DeckZone, String), u32>;
type Cards = BTreeMap<String, CardFacts>;

fn copies_in(deck: &Deck, zone: DeckZone) -> u32 {
    deck.iter()
        .filter(|((z, _), _)| *z == zone)
        .fold(0u32, |all, (_, count)| all.saturating_add(*count))
}

/// The cards of the given zones with their facts, ordered by name, one entry per zone and card.
fn cards_in<'a>(
    deck: &'a Deck,
    cards: &'a Cards,
    zones: &[DeckZone],
) -> Vec<(DeckZone, &'a str, u32, Option<&'a CardFacts>)> {
    let mut found: Vec<_> = deck
        .iter()
        .filter(|((zone, _), _)| zones.contains(zone))
        .map(|((zone, id), count)| (*zone, id.as_str(), *count, cards.get(id)))
        .collect();
    found.sort_by_cached_key(|(zone, id, _, facts)| {
        (name_of(*facts, id).to_lowercase(), (*id).to_owned(), *zone)
    });
    found
}

/// The letters of a color set in WUBRG order; `colorless` for the empty set.
fn letters(set: ColorSet) -> String {
    if set.is_empty() {
        return "colorless".to_owned();
    }
    set.iter()
        .map(|color| match color {
            Color::White => 'W',
            Color::Blue => 'U',
            Color::Black => 'B',
            Color::Red => 'R',
            Color::Green => 'G',
        })
        .collect()
}

/// The words of the type line of the front face, subtypes included.
fn type_words(facts: &CardFacts) -> Vec<String> {
    facts
        .printed
        .faces
        .first()
        .map(|face| &face.type_line)
        .into_iter()
        .flat_map(|line| line.split(|c: char| !c.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

fn has_word(words: &[String], wanted: &str) -> bool {
    words.iter().any(|word| word == wanted)
}

/// The rules text of every face, in lower case.
fn rules_text(facts: &CardFacts) -> String {
    facts
        .printed
        .faces
        .iter()
        .map(|face| face.oracle_text.to_lowercase())
        .collect::<Vec<_>>()
        .join("\n")
}

fn has_keyword(facts: &CardFacts, keyword: &str) -> bool {
    facts
        .printed
        .keywords
        .iter()
        .any(|k| k.eq_ignore_ascii_case(keyword))
}

fn is_basic_land(facts: &CardFacts) -> bool {
    let words = type_words(facts);
    has_word(&words, "Basic") && has_word(&words, "Land")
}

/// The number written out in a "up to nine cards" clause.
fn number_word(word: &str) -> Option<u32> {
    const WORDS: [&str; 20] = [
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    word.parse().ok().or_else(|| {
        WORDS
            .iter()
            .position(|w| *w == word)
            .and_then(|at| u32::try_from(at + 1).ok())
    })
}

/// How many copies of itself a card lets a deck hold, whatever the format says.
enum OwnLimit {
    /// Any number: basic lands, and cards that say so.
    Any,
    /// Up to this many, as the card says.
    UpTo(u32),
    /// The card says nothing; the format decides.
    Unsaid,
}

fn own_limit(facts: &CardFacts) -> OwnLimit {
    if is_basic_land(facts) {
        return OwnLimit::Any;
    }
    let text = rules_text(facts);
    if text.contains("a deck can have any number of cards named") {
        return OwnLimit::Any;
    }
    let marker = "a deck can have up to ";
    text.find(marker)
        .and_then(|at| text[at + marker.len()..].split_whitespace().next())
        .and_then(number_word)
        .map_or(OwnLimit::Unsaid, OwnLimit::UpTo)
}

/// Whether a card can be a commander on its own: a legendary creature, a legendary Vehicle or
/// Spacecraft with a power, or a card that says it can.
fn can_command(facts: &CardFacts) -> bool {
    let words = type_words(facts);
    let has_power = facts
        .printed
        .faces
        .first()
        .is_some_and(|face| face.power.is_some());
    let body = has_word(&words, "Creature")
        || (has_power && (has_word(&words, "Vehicle") || has_word(&words, "Spacecraft")));
    (has_word(&words, "Legendary") && body) || rules_text(facts).contains("can be your commander")
}

fn is_background(facts: &CardFacts) -> bool {
    has_word(&type_words(facts), "Background")
}

fn is_time_lord_doctor(facts: &CardFacts) -> bool {
    let words = type_words(facts);
    has_word(&words, "Time") && has_word(&words, "Lord") && has_word(&words, "Doctor")
}

fn front_name(facts: &CardFacts) -> String {
    facts
        .printed
        .faces
        .first()
        .map_or_else(|| facts.printed.name.clone(), |face| face.name.clone())
}

/// `a` says "Partner with" and names `b`.
fn partners_with(a: &CardFacts, b: &CardFacts) -> bool {
    has_keyword(a, "Partner with")
        && rules_text(a).contains(&format!("partner with {}", front_name(b).to_lowercase()))
}

/// Whether two cards can be commanders together: partners, a named partner, friends forever, a
/// "Choose a Background" creature and a Background, or a Doctor's companion and a Time Lord Doctor.
fn can_pair(a: &CardFacts, b: &CardFacts) -> bool {
    let both = |keyword: &str| has_keyword(a, keyword) && has_keyword(b, keyword);
    let one_way = |x: &CardFacts, y: &CardFacts| {
        (has_keyword(x, "Choose a Background") && is_background(y))
            || (has_keyword(x, "Doctor's companion") && is_time_lord_doctor(y))
    };
    both("Partner")
        || both("Friends forever")
        || partners_with(a, b)
        || partners_with(b, a)
        || one_way(a, b)
        || one_way(b, a)
}

/// The legality of a card in `format`; a format the card has no entry for counts as not legal.
fn legality_in(facts: &CardFacts, format: &str) -> Legality {
    facts
        .legalities
        .get(format)
        .copied()
        .unwrap_or(Legality::NotLegal)
}

/// Banned and not-legal cards of every zone, one issue per card and zone.
fn legal_in_format(deck: &Deck, cards: &Cards, format: &str, issues: &mut Vec<Issue>) {
    for (zone, id, _, facts) in cards_in(deck, cards, &DeckZone::ALL) {
        let Some(facts) = facts else { continue };
        let name = &facts.printed.name;
        match legality_in(facts, format) {
            Legality::Legal | Legality::Restricted => {}
            Legality::Banned => issues.push(Issue::card(
                IssueCode::Banned,
                zone,
                id,
                Some(facts),
                format!("{name} is banned in {format}"),
            )),
            Legality::NotLegal | Legality::Unknown => issues.push(Issue::card(
                IssueCode::NotLegal,
                zone,
                id,
                Some(facts),
                format!("{name} is not legal in {format}"),
            )),
        }
    }
}

/// Cards held in more copies than allowed across `zones`; `default` copies unless the card says more
/// (or fewer, for a restricted card).
fn copy_limits(
    deck: &Deck,
    cards: &Cards,
    zones: &[DeckZone],
    default: u32,
    format: &str,
    issues: &mut Vec<Issue>,
) {
    let mut totals: BTreeMap<&str, (DeckZone, u32)> = BTreeMap::new();
    for (zone, id, count, _) in cards_in(deck, cards, zones) {
        let entry = totals.entry(id).or_insert((zone, 0));
        entry.1 = entry.1.saturating_add(count);
    }
    let mut over: Vec<(String, Issue)> = Vec::new();
    for (id, (zone, copies)) in totals {
        let Some(facts) = cards.get(id) else { continue };
        let limit = match own_limit(facts) {
            OwnLimit::Any => continue,
            OwnLimit::UpTo(n) => n,
            OwnLimit::Unsaid if legality_in(facts, format) == Legality::Restricted => 1,
            OwnLimit::Unsaid => default,
        };
        if copies > limit {
            let name = &facts.printed.name;
            let allowed = if limit == 1 {
                "one copy is".to_owned()
            } else {
                format!("{limit} copies are")
            };
            over.push((
                name.to_lowercase(),
                Issue::card(
                    IssueCode::Copies,
                    zone,
                    id,
                    Some(facts),
                    format!("{name}: {copies} copies, but only {allowed} allowed"),
                ),
            ));
        }
    }
    over.sort_by(|a, b| a.0.cmp(&b.0));
    issues.extend(over.into_iter().map(|(_, issue)| issue));
}

fn companions(deck: &Deck, issues: &mut Vec<Issue>) {
    let count = copies_in(deck, DeckZone::Companion);
    if count > 1 {
        issues.push(Issue::zone(
            IssueCode::CompanionCount,
            DeckZone::Companion,
            format!("a deck has one companion at most, this one has {count}"),
        ));
    }
}

fn commander(deck: &Deck, cards: &Cards, issues: &mut Vec<Issue>) {
    let command = cards_in(deck, cards, &[DeckZone::Commander]);
    let seats = copies_in(deck, DeckZone::Commander);
    if !(1..=2).contains(&seats) {
        issues.push(Issue::zone(
            IssueCode::CommanderCount,
            DeckZone::Commander,
            format!("a Commander deck has one commander, or two that can be partnered; this one has {seats}"),
        ));
    }
    let known: Vec<(&str, &CardFacts)> = command
        .iter()
        .filter_map(|(_, id, _, facts)| facts.map(|facts| (*id, facts)))
        .collect();
    if (1..=2).contains(&seats) {
        for &(id, facts) in &known {
            if !(can_command(facts) || (seats == 2 && is_background(facts))) {
                issues.push(Issue::card(
                    IssueCode::CommanderEligibility,
                    DeckZone::Commander,
                    id,
                    Some(facts),
                    format!(
                        "{} cannot be a commander: it is no legendary creature and does not say it can",
                        facts.printed.name
                    ),
                ));
            }
        }
        if seats == 2 {
            let pair_ok = match known.as_slice() {
                [(_, a), (_, b)] => can_pair(a, b),
                _ => false,
            };
            if !pair_ok {
                issues.push(Issue::zone(
                    IssueCode::CommanderPair,
                    DeckZone::Commander,
                    "these two commanders cannot be partnered: both need Partner, Friends forever, or a Partner with each other, or a Background with Choose a Background",
                ));
            }
        }
    }

    let size = copies_in(deck, DeckZone::Main).saturating_add(seats);
    if size != COMMANDER_SIZE {
        issues.push(Issue::deck(
            IssueCode::DeckSize,
            format!("a Commander deck has exactly {COMMANDER_SIZE} cards with its commanders, this one has {size}"),
        ));
    }
    copy_limits(
        deck,
        cards,
        &[DeckZone::Main, DeckZone::Commander],
        1,
        "commander",
        issues,
    );

    if seats > 0 {
        let identity = known.iter().fold(ColorSet::EMPTY, |all, (_, facts)| {
            facts.color_identity.iter().fold(all, ColorSet::with)
        });
        for (zone, id, _, facts) in cards_in(deck, cards, &[DeckZone::Main, DeckZone::Companion]) {
            let Some(facts) = facts else { continue };
            if !facts.color_identity.is_subset_of(identity) {
                let outside: ColorSet = facts
                    .color_identity
                    .iter()
                    .filter(|color| !identity.contains(*color))
                    .collect();
                issues.push(Issue::card(
                    IssueCode::ColorIdentity,
                    zone,
                    id,
                    Some(facts),
                    format!(
                        "{} has {} in its color identity, outside the commander's ({})",
                        facts.printed.name,
                        letters(outside),
                        letters(identity)
                    ),
                ));
            }
        }
    }
    legal_in_format(deck, cards, "commander", issues);
    companions(deck, issues);
}

fn constructed(deck: &Deck, cards: &Cards, format: &str, issues: &mut Vec<Issue>) {
    let main = copies_in(deck, DeckZone::Main);
    if main < CONSTRUCTED_MIN {
        issues.push(Issue::zone(
            IssueCode::DeckSize,
            DeckZone::Main,
            format!("the main deck needs at least {CONSTRUCTED_MIN} cards, this one has {main}"),
        ));
    }
    let side = copies_in(deck, DeckZone::Sideboard);
    if side > SIDEBOARD_MAX {
        issues.push(Issue::zone(
            IssueCode::SideboardSize,
            DeckZone::Sideboard,
            format!("the sideboard has at most {SIDEBOARD_MAX} cards, this one has {side}"),
        ));
    }
    if copies_in(deck, DeckZone::Commander) > 0 {
        issues.push(Issue::zone(
            IssueCode::NoCommanderZone,
            DeckZone::Commander,
            format!("{format} has no command zone"),
        ));
    }
    copy_limits(
        deck,
        cards,
        &[DeckZone::Main, DeckZone::Sideboard, DeckZone::Companion],
        CONSTRUCTED_COPIES,
        format,
        issues,
    );
    legal_in_format(deck, cards, format, issues);
    companions(deck, issues);
}

/// Every way `deck` breaks the rules of `format`; empty if it follows them.
pub(super) fn check(format: &str, deck: &Deck, cards: &Cards) -> Vec<Issue> {
    if deck.is_empty() {
        return vec![Issue::deck(IssueCode::Empty, "the deck has no cards")];
    }
    let mut issues = Vec::new();
    for (zone, id, _, facts) in cards_in(deck, cards, &DeckZone::ALL) {
        if facts.is_none() {
            issues.push(Issue::card(
                IssueCode::UnknownCard,
                zone,
                id,
                None,
                format!("{id} is not in the card database"),
            ));
        }
    }
    match rules_of(format) {
        None => issues.push(Issue::deck(
            IssueCode::UnknownFormat,
            format!("there are no rules for the format {format:?}"),
        )),
        Some(Rules::Freeform) => {}
        Some(Rules::Commander) => commander(deck, cards, &mut issues),
        Some(Rules::Constructed) => constructed(deck, cards, format, &mut issues),
    }
    issues
}
