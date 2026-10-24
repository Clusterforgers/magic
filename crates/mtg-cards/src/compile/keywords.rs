//! Stage 4: keyword lines. Which words are keywords, and the keywords the DSL declares read with their parameters.

use std::ops::Range;
use std::str::FromStr;

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;

use crate::companion::DeckCondition;
use crate::cost::{Cost, CostItem};
use crate::keyword::{GiftKind, Keyword};
use crate::mana::ManaCostX;
use crate::refs::{Pred, Selector};
use crate::value::{Amount, Cmp, StatKind};

/// What may follow a keyword's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Params {
    /// Nothing.
    None,
    /// Nothing, or a dash and text (`Partner—Friends forever`).
    Dash,
    /// A cost: mana symbols, or a dash and a cost in words.
    Cost,
    /// A number or `X`.
    Number,
    /// Words (`creature`, `from red`).
    Text,
    /// Words that name a kind, then a number or `X` (`3`, `artifact 1`, `Food 3`).
    Counted,
}

/// Every keyword ability the compiler recognises as a keyword line, in lowercase. The ones the
/// DSL declares are a subset (`Keyword::NAMES`); the rest are left to the table.
const KEYWORDS: &[(&str, Params)] = &[
    ("flash", Params::None),
    ("flying", Params::None),
    ("reach", Params::None),
    ("first strike", Params::None),
    ("double strike", Params::None),
    ("deathtouch", Params::None),
    ("trample", Params::None),
    ("lifelink", Params::None),
    ("vigilance", Params::None),
    ("haste", Params::None),
    ("menace", Params::None),
    ("defender", Params::None),
    ("indestructible", Params::None),
    ("hexproof", Params::None),
    ("prowess", Params::None),
    ("convoke", Params::None),
    ("delve", Params::None),
    ("cascade", Params::None),
    ("storm", Params::None),
    ("battle cry", Params::None),
    ("fuse", Params::None),
    ("landwalk", Params::None),
    ("partner", Params::Dash),
    ("companion", Params::Dash),
    ("ward", Params::Cost),
    ("equip", Params::Cost),
    ("reconfigure", Params::Cost),
    ("cycling", Params::Cost),
    ("basic landcycling", Params::Cost),
    ("kicker", Params::Cost),
    ("multikicker", Params::Cost),
    ("flashback", Params::Cost),
    ("madness", Params::Cost),
    ("morph", Params::Cost),
    ("megamorph", Params::Cost),
    ("disguise", Params::Cost),
    ("suspend", Params::Cost),
    ("evoke", Params::Cost),
    ("overload", Params::Cost),
    ("enchant", Params::Text),
    ("enchant player", Params::None),
    ("enchant opponent", Params::None),
    ("partner with", Params::Text),
    ("hexproof from", Params::Text),
    ("protection", Params::Text),
    ("affinity for", Params::Text),
    ("bands with other", Params::Text),
    ("intimidate", Params::None),
    ("fear", Params::None),
    ("shroud", Params::None),
    ("horsemanship", Params::None),
    ("shadow", Params::None),
    ("skulk", Params::None),
    ("banding", Params::None),
    ("infect", Params::None),
    ("wither", Params::None),
    ("persist", Params::None),
    ("undying", Params::None),
    ("exalted", Params::None),
    ("flanking", Params::None),
    ("changeling", Params::None),
    ("devoid", Params::None),
    ("rebound", Params::None),
    ("retrace", Params::None),
    ("split second", Params::None),
    ("haunt", Params::None),
    ("improvise", Params::None),
    ("myriad", Params::None),
    ("evolve", Params::None),
    ("extort", Params::None),
    ("living weapon", Params::None),
    ("for mirrodin!", Params::None),
    ("job select", Params::None),
    ("totem armor", Params::None),
    ("umbra armor", Params::None),
    ("undaunted", Params::None),
    ("soulbond", Params::None),
    ("unleash", Params::None),
    ("decayed", Params::None),
    ("daybound", Params::None),
    ("nightbound", Params::None),
    ("riot", Params::None),
    ("mentor", Params::None),
    ("melee", Params::None),
    ("jump-start", Params::None),
    ("training", Params::None),
    ("read ahead", Params::None),
    ("exploit", Params::None),
    ("ascend", Params::None),
    ("cipher", Params::None),
    ("conspire", Params::None),
    ("dethrone", Params::None),
    ("gravestorm", Params::None),
    ("provoke", Params::None),
    ("double team", Params::None),
    ("compleated", Params::None),
    ("demonstrate", Params::None),
    ("aftermath", Params::None),
    ("ravenous", Params::None),
    ("enlist", Params::None),
    ("bargain", Params::None),
    ("friends forever", Params::None),
    ("choose a background", Params::None),
    ("doctor's companion", Params::None),
    ("bestow", Params::Cost),
    ("buyback", Params::Cost),
    ("dash", Params::Cost),
    ("echo", Params::Cost),
    ("emerge", Params::Cost),
    ("entwine", Params::Cost),
    ("escape", Params::Cost),
    ("escalate", Params::Cost),
    ("miracle", Params::Cost),
    ("mutate", Params::Cost),
    ("ninjutsu", Params::Cost),
    ("prototype", Params::Cost),
    ("replicate", Params::Cost),
    ("scavenge", Params::Cost),
    ("spectacle", Params::Cost),
    ("surge", Params::Cost),
    ("unearth", Params::Cost),
    ("awaken", Params::Cost),
    ("blitz", Params::Cost),
    ("foretell", Params::Cost),
    ("plot", Params::Cost),
    ("prowl", Params::Cost),
    ("mayhem", Params::Cost),
    ("transmute", Params::Cost),
    ("level up", Params::Cost),
    ("embalm", Params::Cost),
    ("sunburst", Params::None),
    ("eternalize", Params::Cost),
    ("outlast", Params::Cost),
    ("cleave", Params::Cost),
    ("encore", Params::Cost),
    ("fortify", Params::Cost),
    ("forecast", Params::Cost),
    ("disturb", Params::Cost),
    ("cumulative upkeep", Params::Cost),
    ("craft", Params::Cost),
    ("squad", Params::Cost),
    ("offspring", Params::Cost),
    ("impending", Params::Cost),
    ("warp", Params::Cost),
    ("harmonize", Params::Cost),
    ("sneak", Params::Cost),
    ("specialize", Params::Cost),
    ("absorb", Params::Number),
    ("afflict", Params::Number),
    ("afterlife", Params::Number),
    ("amplify", Params::Number),
    ("annihilator", Params::Number),
    ("backup", Params::Number),
    ("bloodthirst", Params::Number),
    ("bushido", Params::Number),
    ("casualty", Params::Number),
    ("crew", Params::Number),
    ("dredge", Params::Number),
    ("fabricate", Params::Number),
    ("fading", Params::Number),
    ("graft", Params::Number),
    ("hideaway", Params::Number),
    ("gift", Params::Text),
    ("modular", Params::Number),
    ("poisonous", Params::Number),
    ("rampage", Params::Number),
    ("renown", Params::Number),
    ("ripple", Params::Number),
    ("soulshift", Params::Number),
    ("toxic", Params::Number),
    ("tribute", Params::Number),
    ("vanishing", Params::Number),
    ("devour", Params::Counted),
    ("frenzy", Params::Number),
    ("mobilize", Params::Number),
    ("saddle", Params::Number),
    ("saddled", Params::None),
    ("station", Params::None),
];

/// A keyword found at the start of an item.
struct Found<'a> {
    name: &'static str,
    params: Params,
    /// What follows the name, without the space.
    rest: &'a str,
}

/// Whether `item` starts with `name` as a whole word, ignoring case.
fn starts_with_word(item: &str, name: &str) -> bool {
    item.get(..name.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(name))
        && item
            .get(name.len()..)
            .is_some_and(|tail| tail.is_empty() || tail.starts_with([' ', '\u{2014}']))
}

/// The keyword an item starts with: the longest name that fits, then `...cycling` and `...walk`.
fn find(item: &str) -> Option<Found<'_>> {
    // "Murasame — Equip {5}": the name an Equipment gives an equip ability is flavor.
    if let Some(len) = super::classify::ability_word_len(item)
        && let Some(named) = item
            .get(len..)
            .filter(|rest| starts_with_word(rest, "equip") || starts_with_word(rest, "protection"))
    {
        return find(named);
    }
    let best = KEYWORDS
        .iter()
        .filter(|(name, _)| starts_with_word(item, name))
        .max_by_key(|(name, _)| name.len());
    if let Some(&(name, params)) = best {
        let rest = item.get(name.len()..)?.trim_start();
        return Some(Found { name, params, rest });
    }
    let word = item.split([' ', '\u{2014}']).next()?.to_ascii_lowercase();
    let spelled = |suffix: &str| {
        word.len() > suffix.len()
            && word.ends_with(suffix)
            && word.chars().all(|c| c.is_ascii_alphabetic())
    };
    let (name, params) = if spelled("cycling") {
        ("typecycling", Params::Cost)
    } else if spelled("walk") {
        ("landwalk", Params::None)
    } else {
        return None;
    };
    let rest = item.get(word.len()..)?.trim_start();
    Some(Found { name, params, rest })
}

/// Whether the text after a keyword's name has the shape that keyword takes.
fn plausible(params: Params, rest: &str) -> bool {
    match params {
        Params::None => rest.is_empty(),
        Params::Dash => rest.is_empty() || rest.starts_with('\u{2014}'),
        Params::Cost => rest.contains('{') || rest.starts_with('\u{2014}'),
        Params::Number => {
            rest == "X" || (!rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
        }
        Params::Text => rest.chars().next().is_some_and(char::is_alphabetic),
        Params::Counted => {
            let (words, number) = rest.rsplit_once(' ').unwrap_or(("", rest));
            (number == "X" || (!number.is_empty() && number.chars().all(|c| c.is_ascii_digit())))
                && words.chars().all(|c| c.is_ascii_alphabetic() || c == ' ')
        }
    }
}

/// Whether the text is one keyword with a parameter of its shape.
fn is_item(item: &str) -> bool {
    find(item).is_some_and(|found| plausible(found.params, found.rest))
}

/// The pieces of a line between commas and semicolons, as character ranges without the spaces around them.
fn pieces(chars: &[char]) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    for at in 0..=chars.len() {
        if at == chars.len() || matches!(chars[at], ',' | ';') {
            let (mut first, mut last) = (start, at);
            while first < last && chars[first] == ' ' {
                first += 1;
            }
            while last > first && chars[last - 1] == ' ' {
                last -= 1;
            }
            out.push(first..last);
            start = at + 1;
        }
    }
    out
}

/// The keywords of a line made only of keywords, as character ranges: a list
/// (`Vigilance, deathtouch, haste`) gives one range each, anything else the whole line. `None` when
/// the line is not a keyword line.
pub fn split_items(line: &str) -> Option<Vec<Range<usize>>> {
    let chars: Vec<char> = line.chars().collect();
    let slice = |range: &Range<usize>| chars[range.clone()].iter().collect::<String>();
    let items = pieces(&chars);
    // A partner's name holds a comma, so it is never a list.
    let is_list = items.len() > 1
        && !line.to_ascii_lowercase().starts_with("partner with ")
        && items.iter().all(|range| is_item(&slice(range)));
    if is_list {
        Some(items)
    } else if is_item(line) {
        Some(std::iter::once(0..chars.len()).collect())
    } else {
        None
    }
}

/// Whether the line is made only of keywords.
pub fn is_keyword_line(line: &str) -> bool {
    split_items(line).is_some()
}

/// The keywords an item names: one, a protection from several qualities ("protection from white and from blue") as one
/// protection for each, or "Kicker {W} and/or {2}{B}" as the two kickers; `None` leaves the item to the table.
pub fn read_all(item: &str) -> Option<Vec<Keyword>> {
    let found = find(item)?;
    if found.name == "kicker"
        && let Some((first, second)) = found.rest.split_once(" and/or ")
    {
        let kicker = |rest: &str| spell_cost(rest.trim()).map(Keyword::Kicker);
        return Some(vec![kicker(first)?, kicker(second)?]);
    }
    if found.name == "protection" && plausible(found.params, found.rest) {
        let qualities = protections(found.rest)?;
        return Some(qualities.into_iter().map(Keyword::Protection).collect());
    }
    read(item).map(|keyword| vec![keyword])
}

/// The keyword an item names, when the DSL declares it and its parameter reads; `None` leaves the
/// item to the table.
pub fn read(item: &str) -> Option<Keyword> {
    let found = find(item)?;
    let rest = found.rest;
    if !plausible(found.params, rest) {
        return None;
    }
    match found.name {
        "ward" => cost(rest).map(Keyword::Ward),
        "equip" => equip(rest),
        "reconfigure" => cost(rest).map(Keyword::Reconfigure),
        "cycling" => cost(rest).map(Keyword::Cycling),
        "kicker" => spell_cost(rest).map(Keyword::Kicker),
        "multikicker" => cost(rest).map(Keyword::Multikicker),
        "buyback" => spell_cost(rest).map(Keyword::Buyback),
        "entwine" => spell_cost(rest).map(Keyword::Entwine),
        "flashback" => spell_cost(rest).map(Keyword::Flashback),
        "madness" => spell_cost(rest).map(Keyword::Madness),
        "miracle" => spell_cost(rest).map(Keyword::Miracle),
        "dredge" => positive(rest).map(Keyword::Dredge),
        "morph" => cost(rest).map(Keyword::Morph),
        "megamorph" => cost(rest).map(Keyword::Megamorph),
        "disguise" => cost(rest).map(Keyword::Disguise),
        "evoke" => spell_cost(rest).map(Keyword::Evoke),
        "overload" => cost(rest).map(Keyword::Overload),
        "mutate" => cost(rest).map(Keyword::Mutate),
        "encore" => cost(rest).map(Keyword::Encore),
        "squad" => spell_cost(rest).map(Keyword::Squad),
        "offspring" => spell_cost(rest).map(Keyword::Offspring),
        "casualty" => positive(rest).map(Keyword::Casualty),
        "hideaway" => positive(rest).map(Keyword::Hideaway),
        "gift" => gift(rest),
        "devour" => devour(rest),
        "foretell" => cost(rest).map(Keyword::Foretell),
        "unearth" => spell_cost(rest).map(Keyword::Unearth),
        "embalm" => spell_cost(rest).map(Keyword::Embalm),
        "eternalize" => spell_cost(rest).map(Keyword::Eternalize),
        "scavenge" => spell_cost(rest).map(Keyword::Scavenge),
        "transmute" => spell_cost(rest).map(Keyword::Transmute),
        "awaken" => awaken(rest),
        "escape" => spell_cost(rest).map(Keyword::Escape),
        "dash" => spell_cost(rest).map(Keyword::Dash),
        "bestow" => spell_cost(rest).map(Keyword::Bestow),
        "blitz" => spell_cost(rest).map(Keyword::Blitz),
        "prowl" => spell_cost(rest).map(Keyword::Prowl),
        "mayhem" => spell_cost(rest).map(Keyword::Mayhem),
        "spectacle" => spell_cost(rest).map(Keyword::Spectacle),
        "afterlife" => positive(rest).map(Keyword::Afterlife),
        "ninjutsu" => cost(rest).map(Keyword::Ninjutsu),
        "echo" => spell_cost(rest).map(Keyword::Echo),
        "cumulative upkeep" => spell_cost(rest).map(Keyword::CumulativeUpkeep),
        "vanishing" => positive(rest).map(Keyword::Vanishing),
        "fading" => positive(rest).map(Keyword::Fading),
        "graft" => positive(rest).map(Keyword::Graft),
        "afflict" => positive(rest).map(Keyword::Afflict),
        "replicate" => spell_cost(rest).map(Keyword::Replicate),
        "backup" => positive(rest).map(|count| Keyword::Backup {
            count,
            abilities: Vec::new(),
        }),
        "crew" => positive(rest).map(Keyword::Crew),
        "saddle" => positive(rest).map(Keyword::Saddle),
        "fabricate" => positive(rest).map(Keyword::Fabricate),
        "level up" => cost(rest).map(Keyword::LevelUp),
        "outlast" => cost(rest).map(Keyword::Outlast),
        "suspend" => suspend(rest),
        "enchant" => super::read::enchant_filter(rest).map(Keyword::Enchant),
        "enchant player" => Some(Keyword::EnchantPlayer { opponent: false }),
        "enchant opponent" => Some(Keyword::EnchantPlayer { opponent: true }),
        "partner with" => Some(Keyword::PartnerWith(rest.to_owned())),
        "partner" if rest.starts_with('\u{2014}') => partner_group(rest),
        "protection" => protection(rest).map(Keyword::Protection),
        "companion" => companion(rest),
        "toxic" => positive(rest).map(Keyword::Toxic),
        "annihilator" => positive(rest).map(Keyword::Annihilator),
        "landwalk" => landwalk(item),
        "affinity for" => super::read::affinity_filter(rest).map(Keyword::Affinity),
        "typecycling" => typecycling(item, rest),
        "basic landcycling" => cost(rest).map(|cost| Keyword::Typecycling {
            find: Selector(vec![
                Pred::Type(CardType::Land),
                Pred::Supertype(Supertype::Basic),
            ]),
            cost,
        }),
        name if rest.is_empty() => plain(name),
        _ => None,
    }
}

/// `Devour 2`, `Devour artifact 1`, `Devour Food 3`: the counters for each permanent and what may be sacrificed.
fn devour(rest: &str) -> Option<Keyword> {
    let (words, number) = rest.rsplit_once(' ').unwrap_or(("", rest));
    let of = match words.to_ascii_lowercase().as_str() {
        "" => Pred::Type(CardType::Creature),
        "artifact" => Pred::Type(CardType::Artifact),
        "land" => Pred::Type(CardType::Land),
        "food" => Pred::Subtype(Subtype("Food".to_owned())),
        _ => return None,
    };
    Some(Keyword::Devour {
        counters: positive(number)?,
        of: Selector(vec![of]),
    })
}

/// A whole number above zero, as the counted keywords print it ("Crew 2", "Dredge 3").
fn positive(rest: &str) -> Option<u32> {
    rest.parse().ok().filter(|number| *number > 0)
}

/// `a card`, `a Treasure`, `a tapped Fish`: what the promised gift is.
fn gift(rest: &str) -> Option<Keyword> {
    let kind = match rest.trim().to_ascii_lowercase().as_str() {
        "a card" => GiftKind::Card,
        "a treasure" => GiftKind::Treasure,
        "a food" => GiftKind::Food,
        "a tapped fish" => GiftKind::TappedFish,
        "an octopus" => GiftKind::Octopus,
        "an extra turn" => GiftKind::ExtraTurn,
        _ => return None,
    };
    Some(Keyword::Gift(kind))
}

/// The text of a `Partner—<text>` item (`Friends forever`, `Character select`), the group of commanders that pair.
fn partner_group(rest: &str) -> Option<Keyword> {
    let text = rest
        .trim_start_matches('\u{2014}')
        .trim()
        .trim_end_matches('.')
        .trim();
    (!text.is_empty()).then(|| Keyword::PartnerGroup(text.to_owned()))
}

/// The permanent card types.
const PERMANENT_CARDS: [CardType; 6] = [
    CardType::Artifact,
    CardType::Creature,
    CardType::Enchantment,
    CardType::Land,
    CardType::Planeswalker,
    CardType::Battle,
];

/// The condition of a `Companion—<text>` item for the wordings that read from the card alone (`Each permanent card in
/// your starting deck has mana value 2 or less.`); the other conditions are not read, so the line stays with the table.
fn companion(rest: &str) -> Option<Keyword> {
    let text = rest
        .trim_start_matches('\u{2014}')
        .trim()
        .trim_end_matches('.')
        .trim();
    let lower = text.to_ascii_lowercase();
    let value = |op: Cmp, to: &str| Pred::Compare {
        stat: StatKind::ManaValue,
        op,
        to: Amount::Fixed(to.parse().unwrap_or(0)),
    };
    let number_after = |head: &str, tail: &str| {
        lower
            .strip_prefix(head)
            .and_then(|rest| rest.strip_suffix(tail))
            .filter(|number| number.parse::<u32>().is_ok())
            .map(str::to_owned)
    };
    let condition = if lower == "each nonland card in your starting deck has a different name" {
        DeckCondition::Distinct {
            of: Selector(vec![Pred::Not(Box::new(Pred::Type(CardType::Land)))]),
        }
    } else if let Some(number) = number_after(
        "each permanent card in your starting deck has mana value ",
        " or less",
    ) {
        DeckCondition::Each {
            of: Selector(vec![Pred::AnyOf(
                PERMANENT_CARDS.iter().map(|ty| Pred::Type(*ty)).collect(),
            )]),
            has: Selector(vec![value(Cmp::Le, &number)]),
        }
    } else if let Some(number) = number_after(
        "your starting deck contains only cards with mana value ",
        " or greater and land cards",
    ) {
        DeckCondition::Each {
            of: Selector::default(),
            has: Selector(vec![Pred::AnyOf(vec![
                Pred::Type(CardType::Land),
                value(Cmp::Ge, &number),
            ])]),
        }
    } else if lower
        == "your starting deck contains at least twenty cards more than the minimum deck size"
    {
        DeckCondition::ExtraCards(20)
    } else {
        creature_types(text)?
    };
    Some(Keyword::companion(condition))
}

/// `Each creature card in your starting deck is a Cat, Elemental, or Beast card`: every creature has one of the types.
fn creature_types(text: &str) -> Option<DeckCondition> {
    let list = text
        .strip_prefix("Each creature card in your starting deck is a ")?
        .strip_suffix(" card")?;
    let kinds: Vec<Pred> = list
        .replace(", or ", ", ")
        .replace(" or ", ", ")
        .split(", ")
        .map(|kind| Pred::Subtype(Subtype(kind.trim().to_owned())))
        .collect();
    Some(DeckCondition::Each {
        of: Selector(vec![Pred::Type(CardType::Creature)]),
        has: Selector(vec![Pred::AnyOf(kinds)]),
    })
}

/// The land types a landwalk can name.
const LAND_TYPES: [&str; 6] = ["Plains", "Island", "Swamp", "Mountain", "Forest", "Desert"];

/// A landwalk keyword (`Islandwalk`): the land type it names, as a selector over the lands it looks for.
fn landwalk(item: &str) -> Option<Keyword> {
    let word = item.split([' ', '\u{2014}']).next()?;
    let kind = word.get(..word.len().checked_sub("walk".len())?)?;
    let land = LAND_TYPES
        .iter()
        .find(|land| land.eq_ignore_ascii_case(kind))?;
    Some(Keyword::Landwalk(Selector(vec![Pred::Subtype(Subtype(
        (*land).to_owned(),
    ))])))
}

/// A typecycling keyword (`Plainscycling {2}`, `Landcycling {1}`): the cards it searches for are those of the land
/// type or the land cards it names, and `rest` is its cost.
fn typecycling(item: &str, rest: &str) -> Option<Keyword> {
    let word = item.split([' ', '\u{2014}']).next()?;
    let kind = word.get(..word.len().checked_sub("cycling".len())?)?;
    let find = if kind.eq_ignore_ascii_case("land") {
        Pred::Type(CardType::Land)
    } else {
        let land = LAND_TYPES
            .iter()
            .find(|land| land.eq_ignore_ascii_case(kind))?;
        Pred::Subtype(Subtype((*land).to_owned()))
    };
    Some(Keyword::Typecycling {
        find: Selector(vec![find]),
        cost: cost(rest)?,
    })
}

/// Equip: its cost, or first the creatures it equips beyond the creatures its controller controls ("legendary creature {3}",
/// "commander {2}", "creature token {1}", "Knight {2}"); any other quality stays with the table.
fn equip(rest: &str) -> Option<Keyword> {
    if let Some(cost) = cost(rest) {
        return Some(Keyword::Equip(cost));
    }
    let (words, price) = rest.split_at(rest.find('{')?);
    let only = match words.trim() {
        "legendary creature" => Pred::Supertype(Supertype::Legendary),
        "commander" => Pred::Commander,
        "creature token" => Pred::Token,
        kind if super::read::subtypes::is_subtype(kind) => Pred::Subtype(Subtype(kind.to_owned())),
        _ => return None,
    };
    Some(Keyword::EquipTo {
        to: Box::new(Selector(vec![only])),
        cost: cost(price)?,
    })
}

/// A keyword without parameters.
fn plain(name: &str) -> Option<Keyword> {
    Some(match name {
        "flash" => Keyword::Flash,
        "flying" => Keyword::Flying,
        "reach" => Keyword::Reach,
        "first strike" => Keyword::FirstStrike,
        "double strike" => Keyword::DoubleStrike,
        "deathtouch" => Keyword::Deathtouch,
        "trample" => Keyword::Trample,
        "lifelink" => Keyword::Lifelink,
        "vigilance" => Keyword::Vigilance,
        "haste" => Keyword::Haste,
        "menace" => Keyword::Menace,
        "defender" => Keyword::Defender,
        "indestructible" => Keyword::Indestructible,
        "hexproof" => Keyword::Hexproof,
        "shroud" => Keyword::Shroud,
        "prowess" => Keyword::Prowess,
        "convoke" => Keyword::Convoke,
        "improvise" => Keyword::Improvise,
        "delve" => Keyword::Delve,
        "cascade" => Keyword::Cascade,
        "storm" => Keyword::Storm,
        "gravestorm" => Keyword::Gravestorm,
        "sunburst" => Keyword::Sunburst,
        "demonstrate" => Keyword::Demonstrate,
        "partner" => Keyword::Partner,
        "battle cry" => Keyword::BattleCry,
        "living weapon" => Keyword::LivingWeapon,
        "for mirrodin!" => Keyword::ForMirrodin,
        "job select" => Keyword::JobSelect,
        "fuse" => Keyword::Fuse,
        "changeling" => Keyword::Changeling,
        "devoid" => Keyword::Devoid,
        "infect" => Keyword::Infect,
        "myriad" => Keyword::Myriad,
        "daybound" => Keyword::Daybound,
        "nightbound" => Keyword::Nightbound,
        "doctor's companion" => Keyword::DoctorsCompanion,
        "choose a background" => Keyword::ChooseABackground,
        "wither" => Keyword::Wither,
        "fear" => Keyword::Fear,
        "intimidate" => Keyword::Intimidate,
        "skulk" => Keyword::Skulk,
        "shadow" => Keyword::Shadow,
        "horsemanship" => Keyword::Horsemanship,
        "split second" => Keyword::SplitSecond,
        "undaunted" => Keyword::Undaunted,
        "rebound" => Keyword::Rebound,
        "aftermath" => Keyword::Aftermath,
        "undying" => Keyword::Undying,
        "persist" => Keyword::Persist,
        "exalted" => Keyword::Exalted,
        "melee" => Keyword::Melee,
        "mentor" => Keyword::Mentor,
        "dethrone" => Keyword::Dethrone,
        "umbra armor" => Keyword::UmbraArmor,
        "cipher" => Keyword::Cipher,
        "extort" => Keyword::Extort,
        "evolve" => Keyword::Evolve,
        "saddled" => Keyword::Saddled,
        "station" => Keyword::Station,
        "ravenous" => Keyword::Ravenous,
        "riot" => Keyword::Riot,
        "ascend" => Keyword::Ascend,
        "jump-start" => Keyword::JumpStart,
        "retrace" => Keyword::Retrace,
        _ => return None,
    })
}

/// The quality a protection names, when it is one colour, colourless, multicolored, one kind of permanent, a mana value
/// bound or everything: "from red", "from creatures", "from mana value 3 or less", "from everything". The empty selector
/// matches every source.
fn protection(rest: &str) -> Option<Selector> {
    let word = rest
        .strip_prefix("from ")?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    quality(&word).or_else(|| mana_value_bound(&word))
}

/// The qualities of a protection that names several: "from white and from blue" is one protection for each.
fn protections(rest: &str) -> Option<Vec<Selector>> {
    let words = rest.strip_prefix("from ")?.trim_end_matches('.');
    words
        .split(" and from ")
        .map(|part| protection(&format!("from {part}")))
        .collect()
}

/// "mana value 3 or greater" and "mana value 3 or less": the sources whose mana value is at least, or at most, the number.
fn mana_value_bound(words: &str) -> Option<Selector> {
    let bound = words.strip_prefix("mana value ")?;
    let (number, op) = if let Some(number) = bound.strip_suffix(" or greater") {
        (number, Cmp::Ge)
    } else {
        (bound.strip_suffix(" or less")?, Cmp::Le)
    };
    Some(Selector(vec![Pred::Compare {
        stat: StatKind::ManaValue,
        op,
        to: Amount::Fixed(number.parse().ok()?),
    }]))
}

/// The words of a protection quality, each with the sources it names.
const QUALITIES: [(&str, Pred); 12] = [
    ("white", Pred::Color(Color::White)),
    ("blue", Pred::Color(Color::Blue)),
    ("black", Pred::Color(Color::Black)),
    ("red", Pred::Color(Color::Red)),
    ("green", Pred::Color(Color::Green)),
    ("colorless", Pred::Colorless),
    ("creatures", Pred::Type(CardType::Creature)),
    ("artifacts", Pred::Type(CardType::Artifact)),
    ("enchantments", Pred::Type(CardType::Enchantment)),
    ("planeswalkers", Pred::Type(CardType::Planeswalker)),
    ("lands", Pred::Type(CardType::Land)),
    (
        "multicolored",
        Pred::Compare {
            stat: StatKind::Colors,
            op: Cmp::Ge,
            to: Amount::Fixed(2),
        },
    ),
];

/// The sources a protection quality names, from its word in lowercase ("red", "creatures", "everything"). The empty
/// selector matches every source.
pub fn quality(word: &str) -> Option<Selector> {
    if word == "everything" {
        return Some(Selector::default());
    }
    QUALITIES
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, pred)| Selector(vec![pred.clone()]))
}

/// The sources "each color" names: every source that has a color.
pub fn each_color() -> Selector {
    Selector(vec![Pred::Not(Box::new(Pred::Colorless))])
}

/// The word of a protection quality, the inverse of `quality`.
pub fn quality_word(quality: &Selector) -> Option<&'static str> {
    if *quality == each_color() {
        return Some("each color");
    }
    match quality.0.as_slice() {
        [] => Some("everything"),
        [only] => QUALITIES
            .iter()
            .find(|(_, pred)| pred == only)
            .map(|(name, _)| *name),
        _ => None,
    }
}

/// A cost: mana symbols, or a dash and `Pay N life` or `Discard a card`.
fn cost(rest: &str) -> Option<Cost> {
    if let Some(words) = rest.strip_prefix('\u{2014}') {
        let words = words.trim().trim_end_matches('.');
        if words.eq_ignore_ascii_case("discard a card") {
            return Some(Cost(vec![CostItem::Discard {
                count: Amount::one(),
                filter: None,
            }]));
        }
        let life: i32 = words
            .strip_prefix("Pay ")?
            .strip_suffix(" life")?
            .parse()
            .ok()?;
        return (life > 0).then(|| Cost(vec![CostItem::PayLife(Amount::Fixed(life))]));
    }
    ManaCostX::from_str(rest).ok().map(Cost::mana)
}

/// The cost of a keyword paid as a spell is cast (kicker, buyback, entwine, flashback, evoke, madness): as `cost`, else
/// after a dash any cost an ability pays, in words ("Sacrifice a creature", "{1}{U}, Pay 3 life").
fn spell_cost(rest: &str) -> Option<Cost> {
    cost(rest).or_else(|| {
        let words = rest.strip_prefix('\u{2014}')?.trim().trim_end_matches('.');
        super::read::cost_in_words(words)
    })
}

/// `3—{4}{U}`: the counters and the mana cost of awaken.
fn awaken(rest: &str) -> Option<Keyword> {
    let (count, price) = rest.split_once('\u{2014}')?;
    Some(Keyword::Awaken {
        count: count.trim().parse().ok().filter(|count| *count > 0)?,
        cost: cost(price.trim())?,
    })
}

/// `4—{U}`: the time counters and the mana cost.
fn suspend(rest: &str) -> Option<Keyword> {
    let (time, price) = rest.split_once('\u{2014}')?;
    Some(Keyword::Suspend {
        time: time.trim().parse().ok()?,
        cost: cost(price.trim())?,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: keyword lines are recognised in every wording the cards print, the keywords the DSL declares read with their parameters, and anything else stays unread.
    use super::*;
    use crate::refs::Ref;
    use mtg_core::card::Subtype;
    use rstest::rstest;

    fn walks(land: &str) -> Keyword {
        Keyword::Landwalk(Selector(vec![Pred::Subtype(Subtype(land.to_owned()))]))
    }

    fn mana(text: &str) -> Cost {
        Cost::mana(text.parse().unwrap())
    }

    fn equip_to(only: Pred, cost: &str) -> Keyword {
        Keyword::EquipTo {
            to: Box::new(Selector(vec![only])),
            cost: mana(cost),
        }
    }

    fn finds(preds: Vec<Pred>, cost: &str) -> Keyword {
        Keyword::Typecycling {
            find: Selector(preds),
            cost: mana(cost),
        }
    }

    fn creature() -> Selector {
        Selector(vec![Pred::Type(CardType::Creature)])
    }

    fn multicolored() -> Pred {
        Pred::Compare {
            stat: StatKind::Colors,
            op: Cmp::Ge,
            to: Amount::Fixed(2),
        }
    }

    fn mana_value(op: Cmp, number: i32) -> Pred {
        Pred::Compare {
            stat: StatKind::ManaValue,
            op,
            to: Amount::Fixed(number),
        }
    }

    fn protects(pred: Pred) -> Keyword {
        Keyword::Protection(Selector(vec![pred]))
    }

    #[rstest]
    #[case::one_colour("Protection from white", &[protects(Pred::Color(Color::White))])]
    #[case::two_colours(
        "Protection from white and from blue",
        &[protects(Pred::Color(Color::White)), protects(Pred::Color(Color::Blue))]
    )]
    #[case::lowercase_pair(
        "protection from black and from red.",
        &[protects(Pred::Color(Color::Black)), protects(Pred::Color(Color::Red))]
    )]
    #[case::a_colour_and_a_kind(
        "Protection from red and from creatures",
        &[protects(Pred::Color(Color::Red)), protects(Pred::Type(CardType::Creature))]
    )]
    #[case::three_qualities(
        "Protection from white and from blue and from multicolored",
        &[protects(Pred::Color(Color::White)), protects(Pred::Color(Color::Blue)), protects(multicolored())]
    )]
    #[case::a_bound_after_a_colour(
        "Protection from red and from mana value 2 or less",
        &[protects(Pred::Color(Color::Red)), protects(mana_value(Cmp::Le, 2))]
    )]
    #[case::after_an_ability_word(
        "Void Shields \u{2014} Protection from green and from red",
        &[protects(Pred::Color(Color::Green)), protects(Pred::Color(Color::Red))]
    )]
    #[case::another_keyword("Flying", &[Keyword::Flying])]
    #[case::two_kickers(
        "Kicker {W} and/or {2}{B}",
        &[Keyword::Kicker(mana("{W}")), Keyword::Kicker(mana("{2}{B}"))]
    )]
    fn an_item_reads_into_a_keyword_for_each_quality(
        #[case] item: &str,
        #[case] expected: &[Keyword],
    ) {
        assert_eq!(read_all(item).as_deref(), Some(expected));
    }

    #[rstest]
    #[case::a_subtype_beside_a_colour("Protection from white and from Vampires")]
    #[case::without_the_second_from("Protection from white and blue")]
    #[case::a_missing_quality("Protection from white and from")]
    #[case::a_bound_without_a_side("Protection from mana value 3")]
    #[case::a_bound_without_a_number("Protection from mana value many or less")]
    #[case::not_a_keyword("Draw a card.")]
    #[case::undeclared("Horsemanship and from white")]
    fn an_item_with_a_quality_it_does_not_model_reads_as_no_keyword(#[case] item: &str) {
        assert_eq!(read_all(item), None);
    }

    #[rstest]
    #[case("white")]
    #[case("blue")]
    #[case("black")]
    #[case("red")]
    #[case("green")]
    #[case("colorless")]
    #[case("creatures")]
    #[case("artifacts")]
    #[case("enchantments")]
    #[case("planeswalkers")]
    #[case("lands")]
    #[case("multicolored")]
    #[case("everything")]
    fn a_protection_quality_is_written_as_it_is_read(#[case] word: &str) {
        let quality = quality(word).unwrap();
        assert_eq!(quality_word(&quality), Some(word));
    }

    #[rstest]
    #[case("White")]
    #[case("vampires")]
    #[case("red and blue")]
    #[case("")]
    fn any_other_word_is_no_protection_quality(#[case] word: &str) {
        assert_eq!(quality(word), None);
    }

    #[test]
    fn a_selector_of_two_predicates_has_no_quality_word() {
        let two = Selector(vec![Pred::Colorless, Pred::Type(CardType::Creature)]);
        assert_eq!(quality_word(&two), None);
    }

    #[rstest]
    #[case::flying("Flying", Keyword::Flying)]
    #[case::lowercase("flying", Keyword::Flying)]
    #[case::two_words("First strike", Keyword::FirstStrike)]
    #[case::lowercase_two_words("double strike", Keyword::DoubleStrike)]
    #[case::battle_cry("Battle cry", Keyword::BattleCry)]
    #[case::convoke("Convoke", Keyword::Convoke)]
    #[case::lowercase_convoke("convoke", Keyword::Convoke)]
    #[case::delve("Delve", Keyword::Delve)]
    #[case::lowercase_delve("delve", Keyword::Delve)]
    #[case::hexproof("Hexproof", Keyword::Hexproof)]
    #[case::shroud("Shroud", Keyword::Shroud)]
    #[case::protection_from_a_colour("Protection from white", Keyword::Protection(Selector(vec![Pred::Color(Color::White)])))]
    #[case::protection_from_a_colour_in_lowercase("protection from red", Keyword::Protection(Selector(vec![Pred::Color(Color::Red)])))]
    #[case::protection_from_colorless("Protection from colorless", Keyword::Protection(Selector(vec![Pred::Colorless])))]
    #[case::protection_from_a_kind("Protection from creatures", Keyword::Protection(Selector(vec![Pred::Type(CardType::Creature)])))]
    #[case::protection_from_everything(
        "Protection from everything",
        Keyword::Protection(Selector::default())
    )]
    #[case::protection_from_multicolored("Protection from multicolored", protects(multicolored()))]
    #[case::protection_from_a_high_mana_value(
        "protection from mana value 3 or greater",
        protects(mana_value(Cmp::Ge, 3))
    )]
    #[case::protection_from_a_low_mana_value_after_an_ability_word(
        "Void Shields \u{2014} Protection from mana value 3 or less",
        protects(mana_value(Cmp::Le, 3))
    )]
    #[case::partner("Partner", Keyword::Partner)]
    #[case::fuse("Fuse", Keyword::Fuse)]
    #[case::daybound("Daybound", Keyword::Daybound)]
    #[case::nightbound("nightbound", Keyword::Nightbound)]
    #[case::wither("Wither", Keyword::Wither)]
    #[case::toxic("Toxic 1", Keyword::Toxic(1))]
    #[case::toxic_three("Toxic 3", Keyword::Toxic(3))]
    #[case::fear("fear", Keyword::Fear)]
    #[case::intimidate("Intimidate", Keyword::Intimidate)]
    #[case::skulk("Skulk", Keyword::Skulk)]
    #[case::shadow("Shadow", Keyword::Shadow)]
    #[case::horsemanship("Horsemanship", Keyword::Horsemanship)]
    #[case::lowercase_horsemanship("horsemanship", Keyword::Horsemanship)]
    #[case::split_second("Split second", Keyword::SplitSecond)]
    #[case::undaunted("Undaunted", Keyword::Undaunted)]
    #[case::islandwalk("Islandwalk", walks("Island"))]
    #[case::forestwalk("forestwalk", walks("Forest"))]
    #[case::desertwalk("Desertwalk", walks("Desert"))]
    #[case::equip("Equip {1}", Keyword::Equip(mana("{1}")))]
    #[case::equip_two("Equip {2}{G}", Keyword::Equip(mana("{2}{G}")))]
    #[case::equip_legendary(
        "Equip legendary creature {3}",
        equip_to(Pred::Supertype(Supertype::Legendary), "{3}")
    )]
    #[case::equip_commander("Equip commander {2}", equip_to(Pred::Commander, "{2}"))]
    #[case::equip_token("Equip creature token {1}", equip_to(Pred::Token, "{1}"))]
    #[case::equip_a_type("Equip Knight {2}", equip_to(Pred::Subtype(Subtype("Knight".into())), "{2}"))]
    #[case::equip_a_named_legendary(
        "Gae Bolg \u{2014} Equip legendary creature {4}",
        equip_to(Pred::Supertype(Supertype::Legendary), "{4}")
    )]
    #[case::kicker("Kicker {4}", Keyword::Kicker(mana("{4}")))]
    #[case::multikicker("Multikicker {1}{G}", Keyword::Multikicker(mana("{1}{G}")))]
    #[case::flashback("Flashback {2}{R}", Keyword::Flashback(mana("{2}{R}")))]
    #[case::evoke("Evoke {2}{U}", Keyword::Evoke(mana("{2}{U}")))]
    #[case::buyback("Buyback {3}", Keyword::Buyback(mana("{3}")))]
    #[case::entwine("Entwine {2}", Keyword::Entwine(mana("{2}")))]
    #[case::kicker_sacrificing(
        "Kicker\u{2014}Sacrifice a creature.",
        Keyword::Kicker(Cost(vec![CostItem::Sacrifice {
            filter: Selector(vec![Pred::Type(CardType::Creature)]),
            count: Amount::one(),
        }]))
    )]
    #[case::kicker_in_life("Kicker\u{2014}Pay 3 life.", Keyword::Kicker(Cost(vec![CostItem::PayLife(Amount::Fixed(3))])))]
    #[case::buyback_in_life("Buyback\u{2014}Pay 4 life.", Keyword::Buyback(Cost(vec![CostItem::PayLife(Amount::Fixed(4))])))]
    #[case::evoke_exiling_a_card_from_the_hand(
        "Evoke\u{2014}Exile a red card from your hand.",
        Keyword::Evoke(Cost(vec![CostItem::ExileFromHand {
            filter: Selector(vec![Pred::Color(Color::Red)]),
            count: Amount::one(),
        }]))
    )]
    #[case::flashback_with_mana_and_life(
        "Flashback\u{2014}{1}{U}, Pay 3 life.",
        Keyword::Flashback(Cost(vec![CostItem::Mana("{1}{U}".parse().unwrap()), CostItem::PayLife(Amount::Fixed(3))]))
    )]
    #[case::madness_with_mana_and_life(
        "Madness\u{2014}{2}{B}, Pay 8 life.",
        Keyword::Madness(Cost(vec![CostItem::Mana("{2}{B}".parse().unwrap()), CostItem::PayLife(Amount::Fixed(8))]))
    )]
    #[case::cycling("Cycling {2}", Keyword::Cycling(mana("{2}")))]
    #[case::plainscycling("Plainscycling {2}", finds(vec![Pred::Subtype(Subtype("Plains".into()))], "{2}"))]
    #[case::lowercase_forestcycling("forestcycling {1}", finds(vec![Pred::Subtype(Subtype("Forest".into()))], "{1}"))]
    #[case::landcycling("Landcycling {1}", finds(vec![Pred::Type(CardType::Land)], "{1}"))]
    #[case::basic_landcycling(
        "Basic landcycling {1}{G}",
        finds(vec![Pred::Type(CardType::Land), Pred::Supertype(Supertype::Basic)], "{1}{G}")
    )]
    #[case::improvise("Improvise", Keyword::Improvise)]
    #[case::affinity_for_artifacts("Affinity for artifacts", Keyword::Affinity(Selector(vec![Pred::Type(CardType::Artifact)])))]
    #[case::affinity_for_a_subtype("Affinity for Islands", Keyword::Affinity(Selector(vec![Pred::Subtype(Subtype("Island".into()))])))]
    #[case::affinity_for_tokens("Affinity for tokens", Keyword::Affinity(Selector(vec![Pred::Token])))]
    #[case::affinity_for_equipment(
        "Affinity for Equipment",
        Keyword::Affinity(Selector(vec![Pred::Subtype(Subtype("Equipment".into()))]))
    )]
    #[case::affinity_for_artifact_creatures(
        "Affinity for artifact creatures",
        Keyword::Affinity(Selector(vec![Pred::Type(CardType::Artifact), Pred::Type(CardType::Creature)]))
    )]
    #[case::affinity_for_historic_permanents(
        "Affinity for historic permanents",
        Keyword::Affinity(Selector(vec![Pred::AnyOf(vec![
            Pred::Type(CardType::Artifact),
            Pred::Supertype(Supertype::Legendary),
            Pred::Subtype(Subtype("Saga".into())),
        ])]))
    )]
    #[case::madness("Madness {R}", Keyword::Madness(mana("{R}")))]
    #[case::miracle("Miracle {W}", Keyword::Miracle(mana("{W}")))]
    #[case::dredge("Dredge 3", Keyword::Dredge(3))]
    #[case::dredge_six("Dredge 6", Keyword::Dredge(6))]
    #[case::morph("Morph {3}{B}", Keyword::Morph(mana("{3}{B}")))]
    #[case::megamorph("Megamorph {1}{G}", Keyword::Megamorph(mana("{1}{G}")))]
    #[case::disguise("Disguise {1}{W}", Keyword::Disguise(mana("{1}{W}")))]
    #[case::overload("Overload {3}{U}{U}", Keyword::Overload(mana("{3}{U}{U}")))]
    #[case::reconfigure("Reconfigure {2}", Keyword::Reconfigure(mana("{2}")))]
    #[case::mutate("Mutate {1}{G}{G}", Keyword::Mutate(mana("{1}{G}{G}")))]
    #[case::encore("Encore {5}{R}", Keyword::Encore(mana("{5}{R}")))]
    #[case::squad("Squad {2}", Keyword::Squad(mana("{2}")))]
    #[case::replicate("Replicate {2}", Keyword::Replicate(mana("{2}")))]
    #[case::offspring("Offspring {2}", Keyword::Offspring(mana("{2}")))]
    #[case::casualty("Casualty 2", Keyword::Casualty(2))]
    #[case::hideaway("Hideaway 4", Keyword::Hideaway(4))]
    #[case::gift_a_card("Gift a card", Keyword::Gift(GiftKind::Card))]
    #[case::gift_a_treasure("Gift a Treasure", Keyword::Gift(GiftKind::Treasure))]
    #[case::gift_a_tapped_fish("Gift a tapped Fish", Keyword::Gift(GiftKind::TappedFish))]
    #[case::gift_an_extra_turn("Gift an extra turn", Keyword::Gift(GiftKind::ExtraTurn))]
    #[case::hideaway_five("hideaway 5", Keyword::Hideaway(5))]
    #[case::demonstrate("Demonstrate", Keyword::Demonstrate)]
    #[case::devour("Devour 2", Keyword::Devour { counters: 2, of: creature() })]
    #[case::devour_artifact("Devour artifact 1", Keyword::Devour { counters: 1, of: Selector(vec![Pred::Type(CardType::Artifact)]) })]
    #[case::devour_land_in_lowercase("devour land 3", Keyword::Devour { counters: 3, of: Selector(vec![Pred::Type(CardType::Land)]) })]
    #[case::devour_food("Devour Food 3", Keyword::Devour { counters: 3, of: Selector(vec![Pred::Subtype(Subtype("Food".to_owned()))]) })]
    #[case::squad_discarding(
        "Squad\u{2014}{1}, Discard a card.",
        Keyword::Squad(Cost(vec![CostItem::Mana("{1}".parse().unwrap()), CostItem::Discard { count: Amount::one(), filter: None }]))
    )]
    #[case::echo("Echo {2}{R}", Keyword::Echo(mana("{2}{R}")))]
    #[case::echo_lowercase("echo {G}", Keyword::Echo(mana("{G}")))]
    #[case::echo_discarding(
        "Echo\u{2014}Discard a card.",
        Keyword::Echo(Cost(vec![CostItem::Discard { count: Amount::one(), filter: None }]))
    )]
    #[case::echo_sacrificing(
        "Echo\u{2014}Sacrifice two lands.",
        Keyword::Echo(Cost(vec![CostItem::Sacrifice {
            filter: Selector(vec![Pred::Type(CardType::Land)]),
            count: Amount::Fixed(2),
        }]))
    )]
    #[case::cumulative_upkeep("Cumulative upkeep {1}", Keyword::CumulativeUpkeep(mana("{1}")))]
    #[case::cumulative_upkeep_two_colours(
        "Cumulative upkeep {U}{R}",
        Keyword::CumulativeUpkeep(mana("{U}{R}"))
    )]
    #[case::cumulative_upkeep_in_life(
        "Cumulative upkeep\u{2014}Pay 1 life.",
        Keyword::CumulativeUpkeep(Cost(vec![CostItem::PayLife(Amount::Fixed(1))]))
    )]
    #[case::cumulative_upkeep_sacrificing(
        "Cumulative upkeep\u{2014}Sacrifice a creature.",
        Keyword::CumulativeUpkeep(Cost(vec![CostItem::Sacrifice {
            filter: creature(),
            count: Amount::one(),
        }]))
    )]
    #[case::undying("Undying", Keyword::Undying)]
    #[case::persist("persist", Keyword::Persist)]
    #[case::exalted("Exalted", Keyword::Exalted)]
    #[case::melee("Melee", Keyword::Melee)]
    #[case::mentor("Mentor", Keyword::Mentor)]
    #[case::dethrone("dethrone", Keyword::Dethrone)]
    #[case::umbra_armor("Umbra armor", Keyword::UmbraArmor)]
    #[case::cipher("Cipher", Keyword::Cipher)]
    #[case::graft("Graft 2", Keyword::Graft(2))]
    #[case::afflict("Afflict 3", Keyword::Afflict(3))]
    #[case::extort("Extort", Keyword::Extort)]
    #[case::evolve("Evolve", Keyword::Evolve)]
    #[case::annihilator("Annihilator 2", Keyword::Annihilator(2))]
    #[case::annihilator_six("Annihilator 6", Keyword::Annihilator(6))]
    #[case::ravenous("Ravenous", Keyword::Ravenous)]
    #[case::riot("Riot", Keyword::Riot)]
    #[case::ascend("Ascend", Keyword::Ascend)]
    #[case::fabricate("Fabricate 2", Keyword::Fabricate(2))]
    #[case::fabricate_lowercase("fabricate 1", Keyword::Fabricate(1))]
    #[case::foretell("Foretell {1}{U}", Keyword::Foretell(mana("{1}{U}")))]
    #[case::foretell_for_nothing("Foretell {0}", Keyword::Foretell(mana("{0}")))]
    #[case::unearth("Unearth {1}{B}", Keyword::Unearth(mana("{1}{B}")))]
    #[case::unearth_in_mana_only_with_reminder("unearth {B}", Keyword::Unearth(mana("{B}")))]
    #[case::rebound("Rebound", Keyword::Rebound)]
    #[case::jump_start("Jump-start", Keyword::JumpStart)]
    #[case::retrace("retrace", Keyword::Retrace)]
    #[case::embalm("Embalm {3}{W}", Keyword::Embalm(mana("{3}{W}")))]
    #[case::eternalize("Eternalize {4}{U}{U}", Keyword::Eternalize(mana("{4}{U}{U}")))]
    #[case::scavenge("Scavenge {3}{G}", Keyword::Scavenge(mana("{3}{G}")))]
    #[case::scavenge_free("Scavenge {0}", Keyword::Scavenge(mana("{0}")))]
    #[case::transmute("Transmute {1}{U}{U}", Keyword::Transmute(mana("{1}{U}{U}")))]
    #[case::awaken("Awaken 3\u{2014}{4}{U}", Keyword::Awaken { count: 3, cost: mana("{4}{U}") })]
    #[case::awaken_big("Awaken 6\u{2014}{6}{U}{U}{U}", Keyword::Awaken { count: 6, cost: mana("{6}{U}{U}{U}") })]
    #[case::gravestorm("Gravestorm", Keyword::Gravestorm)]
    #[case::sunburst("Sunburst", Keyword::Sunburst)]
    #[case::eternalize_with_a_life_cost(
        "Eternalize—Pay 3 life.",
        Keyword::Eternalize(Cost(vec![CostItem::PayLife(Amount::Fixed(3))]))
    )]
    #[case::aftermath("Aftermath", Keyword::Aftermath)]
    #[case::dash("Dash {2}{R}", Keyword::Dash(mana("{2}{R}")))]
    #[case::bestow("Bestow {3}{G}{G}", Keyword::Bestow(mana("{3}{G}{G}")))]
    #[case::bestow_five_colors(
        "Bestow {2}{W}{U}{B}{R}{G}",
        Keyword::Bestow(mana("{2}{W}{U}{B}{R}{G}"))
    )]
    #[case::blitz("Blitz {2}{R}", Keyword::Blitz(mana("{2}{R}")))]
    #[case::prowl("Prowl {1}{B}", Keyword::Prowl(mana("{1}{B}")))]
    #[case::mayhem("Mayhem {1}{B}", Keyword::Mayhem(mana("{1}{B}")))]
    #[case::mayhem_hybrid("Mayhem {B/R}", Keyword::Mayhem(mana("{B/R}")))]
    #[case::spectacle("Spectacle {1}{B}", Keyword::Spectacle(mana("{1}{B}")))]
    #[case::spectacle_hybrid("spectacle {B/R}{B/R}", Keyword::Spectacle(mana("{B/R}{B/R}")))]
    #[case::afterlife("Afterlife 1", Keyword::Afterlife(1))]
    #[case::afterlife_three("afterlife 3", Keyword::Afterlife(3))]
    #[case::ninjutsu("Ninjutsu {1}{U}", Keyword::Ninjutsu(mana("{1}{U}")))]
    #[case::ninjutsu_for_one_mana("ninjutsu {B}", Keyword::Ninjutsu(mana("{B}")))]
    #[case::escape_exiling_cards(
        "Escape\u{2014}{2}{R}, Exile three other cards from your graveyard.",
        Keyword::Escape(Cost(vec![
            CostItem::Mana("{2}{R}".parse().unwrap()),
            CostItem::ExileFromGraveyard { filter: Selector(vec![Pred::Other]), count: Amount::Fixed(3) },
        ]))
    )]
    #[case::crew("Crew 3", Keyword::Crew(3))]
    #[case::vanishing("Vanishing 3", Keyword::Vanishing(3))]
    #[case::fading("Fading 5", Keyword::Fading(5))]
    #[case::backup("Backup 2", Keyword::Backup { count: 2, abilities: vec![] })]
    #[case::saddle("Saddle 2", Keyword::Saddle(2))]
    #[case::station("Station", Keyword::Station)]
    #[case::for_mirrodin("For Mirrodin!", Keyword::ForMirrodin)]
    #[case::job_select("Job select", Keyword::JobSelect)]
    #[case::a_named_equip("Murasame \u{2014} Equip {5}", Keyword::Equip(mana("{5}")))]
    #[case::a_named_equip_of_two_words(
        "Lightbringer and Hero's Shield \u{2014} Equip {4}",
        Keyword::Equip(mana("{4}"))
    )]
    #[case::level_up("Level up {1}{U}", Keyword::LevelUp(mana("{1}{U}")))]
    #[case::level_up_for_one("level up {1}", Keyword::LevelUp(mana("{1}")))]
    #[case::outlast("Outlast {W}", Keyword::Outlast(mana("{W}")))]
    #[case::outlast_two_symbols("Outlast {1}{W}", Keyword::Outlast(mana("{1}{W}")))]
    #[case::changeling("Changeling", Keyword::Changeling)]
    #[case::devoid("Devoid", Keyword::Devoid)]
    #[case::infect("Infect", Keyword::Infect)]
    #[case::myriad("Myriad", Keyword::Myriad)]
    #[case::ward_mana("Ward {2}", Keyword::Ward(mana("{2}")))]
    #[case::ward_life(
        "Ward\u{2014}Pay 3 life.",
        Keyword::Ward(Cost(vec![CostItem::PayLife(Amount::Fixed(3))]))
    )]
    #[case::cycling_in_life(
        "Cycling\u{2014}Pay 2 life.",
        Keyword::Cycling(Cost(vec![CostItem::PayLife(Amount::Fixed(2))]))
    )]
    #[case::ward_discard(
        "Ward\u{2014}Discard a card.",
        Keyword::Ward(Cost(vec![CostItem::Discard { count: Amount::one(), filter: None }]))
    )]
    #[case::suspend("Suspend 4\u{2014}{U}", Keyword::Suspend { time: 4, cost: mana("{U}") })]
    #[case::enchant("Enchant creature", Keyword::Enchant(creature()))]
    #[case::enchant_land("Enchant land", Keyword::Enchant(Selector(vec![Pred::Type(CardType::Land)])))]
    #[case::enchant_a_player("Enchant player", Keyword::EnchantPlayer { opponent: false })]
    #[case::enchant_an_opponent("Enchant opponent", Keyword::EnchantPlayer { opponent: true })]
    #[case::enchant_your_creature(
        "Enchant creature you control",
        Keyword::Enchant(Selector(vec![Pred::Type(CardType::Creature), Pred::ControlledBy(Ref::You)]))
    )]
    #[case::enchant_their_creature(
        "Enchant creature an opponent controls",
        Keyword::Enchant(Selector(vec![Pred::Type(CardType::Creature), Pred::ControlledBy(Ref::Opponents)]))
    )]
    #[case::enchant_a_permanent("Enchant permanent", Keyword::Enchant(Selector::default()))]
    #[case::enchant_a_nonland_permanent(
        "Enchant nonland permanent",
        Keyword::Enchant(Selector(vec![Pred::Not(Box::new(Pred::Type(CardType::Land)))]))
    )]
    #[case::enchant_two_types(
        "Enchant artifact or land",
        Keyword::Enchant(Selector(vec![Pred::AnyOf(vec![Pred::Type(CardType::Artifact), Pred::Type(CardType::Land)])]))
    )]
    #[case::enchant_three_types(
        "Enchant creature, land, or planeswalker",
        Keyword::Enchant(Selector(vec![Pred::AnyOf(vec![
            Pred::Type(CardType::Creature),
            Pred::Type(CardType::Land),
            Pred::Type(CardType::Planeswalker),
        ])]))
    )]
    #[case::enchant_your_artifact(
        "Enchant artifact you control",
        Keyword::Enchant(Selector(vec![Pred::Type(CardType::Artifact), Pred::ControlledBy(Ref::You)]))
    )]
    #[case::enchant_a_subtype(
        "Enchant Forest",
        Keyword::Enchant(Selector(vec![Pred::Subtype(Subtype("Forest".into()))]))
    )]
    #[case::companion(
        "Companion \u{2014} Each nonland card in your starting deck has a different name.",
        Keyword::companion(DeckCondition::Distinct {
            of: Selector(vec![Pred::Not(Box::new(Pred::Type(CardType::Land)))])
        })
    )]
    #[case::companion_permanent_value(
        "Companion \u{2014} Each permanent card in your starting deck has mana value 2 or less.",
        Keyword::companion(DeckCondition::Each {
            of: Selector(vec![Pred::AnyOf(PERMANENT_CARDS.iter().map(|ty| Pred::Type(*ty)).collect())]),
            has: Selector(vec![Pred::Compare {
                stat: StatKind::ManaValue,
                op: Cmp::Le,
                to: Amount::Fixed(2)
            }])
        })
    )]
    #[case::companion_land_or_big(
        "Companion \u{2014} Your starting deck contains only cards with mana value 3 or greater and land cards.",
        Keyword::companion(DeckCondition::Each {
            of: Selector::default(),
            has: Selector(vec![Pred::AnyOf(vec![
                Pred::Type(CardType::Land),
                Pred::Compare { stat: StatKind::ManaValue, op: Cmp::Ge, to: Amount::Fixed(3) }
            ])])
        })
    )]
    #[case::companion_creature_types(
        "Companion \u{2014} Each creature card in your starting deck is a Cat, Elemental, Nightmare, Dinosaur, or Beast card.",
        Keyword::companion(DeckCondition::Each {
            of: Selector(vec![Pred::Type(CardType::Creature)]),
            has: Selector(vec![Pred::AnyOf(
                ["Cat", "Elemental", "Nightmare", "Dinosaur", "Beast"]
                    .map(|name| Pred::Subtype(Subtype(name.into())))
                    .to_vec()
            )])
        })
    )]
    #[case::companion_extra_cards(
        "Companion \u{2014} Your starting deck contains at least twenty cards more than the minimum deck size.",
        Keyword::companion(DeckCondition::ExtraCards(20))
    )]
    #[case::partner_with(
        "Partner with Pir, Imaginative Rascal",
        Keyword::PartnerWith("Pir, Imaginative Rascal".into())
    )]
    #[case::partner_friends(
        "Partner\u{2014}Friends forever",
        Keyword::PartnerGroup("Friends forever".into())
    )]
    #[case::partner_character_select(
        "Partner\u{2014}Character select",
        Keyword::PartnerGroup("Character select".into())
    )]
    #[case::partner_survivors("partner\u{2014}Survivors.", Keyword::PartnerGroup("Survivors".into()))]
    #[case::partner_father_and_son(
        "Partner\u{2014}Father & son",
        Keyword::PartnerGroup("Father & son".into())
    )]
    #[case::doctors_companion("Doctor's companion", Keyword::DoctorsCompanion)]
    #[case::lowercase_doctors_companion("doctor's companion", Keyword::DoctorsCompanion)]
    #[case::choose_a_background("Choose a Background", Keyword::ChooseABackground)]
    fn declared_keywords_are_read(#[case] item: &str, #[case] expected: Keyword) {
        assert_eq!(read(item), Some(expected));
    }

    #[rstest]
    #[case::two_kicker_costs("Kicker {1}{R} and/or {2}{G}")]
    #[case::equip_for_a_quality_it_does_not_name("Equip worthy {2}")]
    #[case::equip_for_a_made_up_type("Equip Banana {2}")]
    #[case::equip_for_two_types("Equip Shaman, Warlock, or Wizard {2}")]
    #[case::equip_without_a_cost("Equip legendary creature")]
    #[case::hexproof_from("Hexproof from white")]
    #[case::protection_from_a_pair("Protection from red and from blue")]
    #[case::protection_from_a_subtype("Protection from Vampires")]
    #[case::protection_without_a_quality("Protection from")]
    #[case::cycling_by_sacrifice("Cycling\u{2014}Sacrifice a land.")]
    #[case::ward_sacrifice("Ward\u{2014}Sacrifice a permanent.")]
    #[case::enchant_a_card_in_a_graveyard("Enchant creature card in a graveyard")]
    #[case::enchant_with_a_quality_the_table_keeps("Enchant noncommander creature")]
    #[case::enchant_with_another_aura("Enchant creature with another Aura attached to it")]
    #[case::enchant_nothing("Enchant")]
    #[case::partner_without_a_text("Partner\u{2014}")]
    #[case::doctors_companion_with_a_cost("Doctor's companion {1}")]
    #[case::companion_with_even_values(
        "Companion \u{2014} Your starting deck contains only cards with even mana values."
    )]
    #[case::companion_with_activated_abilities(
        "Companion \u{2014} Each permanent card in your starting deck has an activated ability."
    )]
    #[case::companion_without_a_condition("Companion")]
    #[case::undeclared("Banding")]
    #[case::crew_x("Crew X")]
    #[case::dredge_x("Dredge X")]
    #[case::dredge_nothing("Dredge 0")]
    #[case::fabricate_x("Fabricate X")]
    #[case::fabricate_nothing("Fabricate 0")]
    #[case::afterlife_x("Afterlife X")]
    #[case::afterlife_nothing("Afterlife 0")]
    #[case::horsemanship_with_a_cost("Horsemanship {1}")]
    #[case::spectacle_without_a_cost("Spectacle")]
    #[case::ascend_with_a_number("Ascend 1")]
    #[case::crew_nothing("Crew 0")]
    #[case::saddle_nothing("Saddle 0")]
    #[case::saddle_x("Saddle X")]
    #[case::outlast_without_a_cost("Outlast")]
    #[case::mutate_without_a_cost("Mutate")]
    #[case::a_parameter_on_changeling("Changeling {1}")]
    #[case::a_type_that_is_not_a_land_type("Wizardcycling {2}")]
    #[case::landwalk_of_a_type_that_is_not_a_land_type("Cowwalk")]
    #[case::parameter_on_a_plain_keyword("Flying {2}")]
    #[case::suspend_without_time("Suspend {U}")]
    #[case::not_a_keyword("Draw a card.")]
    fn other_items_stay_unread(#[case] item: &str) {
        assert_eq!(read(item), None);
    }

    #[rstest]
    #[case::single("Flying", &["Flying"])]
    #[case::list("Vigilance, deathtouch, haste", &["Vigilance", "deathtouch", "haste"])]
    #[case::two_words_in_a_list("Flying, first strike, lifelink", &["Flying", "first strike", "lifelink"])]
    #[case::parameters_in_a_list("Flying, cycling {2}", &["Flying", "cycling {2}"])]
    #[case::semicolons("Flying; first strike", &["Flying", "first strike"])]
    #[case::undeclared_in_a_list("Flying, horsemanship", &["Flying", "horsemanship"])]
    #[case::cost_in_words("Ward\u{2014}Pay 3 life.", &["Ward\u{2014}Pay 3 life."])]
    #[case::cost_with_a_comma("Ward\u{2014}{2}, Pay 2 life.", &["Ward\u{2014}{2}, Pay 2 life."])]
    #[case::partner_name_with_a_comma("Partner with Pir, Imaginative Rascal", &["Partner with Pir, Imaginative Rascal"])]
    #[case::protection("Protection from red and from blue", &["Protection from red and from blue"])]
    #[case::level_up("Level up {1}{U}", &["Level up {1}{U}"])]
    #[case::typecycling("Plainscycling {2}", &["Plainscycling {2}"])]
    #[case::number("Annihilator 2", &["Annihilator 2"])]
    #[case::x_number("Bloodthirst X", &["Bloodthirst X"])]
    fn keyword_lines_split_into_their_items(#[case] line: &str, #[case] expected: &[&str]) {
        let chars: Vec<char> = line.chars().collect();
        let items: Vec<String> = split_items(line)
            .unwrap()
            .into_iter()
            .map(|range| chars[range].iter().collect())
            .collect();
        assert_eq!(items, expected);
        assert!(is_keyword_line(line));
    }

    #[rstest]
    #[case::keyword_as_a_subject("Flying creatures you control get +1/+1.")]
    #[case::keyword_with_a_sentence("Flash is great.")]
    #[case::trailing_comma("Flying,")]
    #[case::unknown_in_a_list("Flying, draw a card")]
    #[case::enchanted_creature("Enchanted creature gets +2/+2.")]
    #[case::equipped_creature("Equipped creature has flying.")]
    #[case::sentence("Draw a card.")]
    #[case::empty("")]
    #[case::number_with_words("Crew a lot")]
    #[case::cost_without_symbols("Equip a creature")]
    fn other_lines_are_not_keyword_lines(#[case] line: &str) {
        assert_eq!(split_items(line), None);
        assert!(!is_keyword_line(line));
    }

    /// The name a keyword is printed with when it is not the DSL's: every "Partner—text" is a `Partner` line.
    fn printed(name: &str) -> &str {
        match name {
            "Partner group" => "Partner",
            "Affinity" => "Affinity for",
            // Every typecycling is read from the word before `cycling`; `Basic landcycling` is the one with a table entry.
            "Typecycling" => "Basic landcycling",
            "Equip to" => "Equip",
            other => other,
        }
    }

    #[test]
    fn every_keyword_the_dsl_declares_is_recognised_by_name() {
        for name in Keyword::NAMES.iter().copied() {
            let name = printed(name);
            assert!(
                KEYWORDS
                    .iter()
                    .any(|(known, _)| known.eq_ignore_ascii_case(name)),
                "{name} is missing from the keyword table"
            );
        }
    }

    // One sample line for each keyword of the DSL.
    #[allow(clippy::too_many_lines)]
    #[test]
    fn a_keyword_read_carries_the_name_it_was_printed_with() {
        let samples = [
            "Flash",
            "Flying",
            "Reach",
            "First strike",
            "Double strike",
            "Deathtouch",
            "Trample",
            "Lifelink",
            "Vigilance",
            "Haste",
            "Menace",
            "Defender",
            "Indestructible",
            "Hexproof",
            "Shroud",
            "Protection from red",
            "Prowess",
            "Convoke",
            "Delve",
            "Cascade",
            "Storm",
            "Partner",
            "Battle cry",
            "Fuse",
            "Ward {1}",
            "Equip {1}",
            "Reconfigure {1}",
            "Enchant creature",
            "Cycling {1}",
            "Kicker {1}",
            "Multikicker {1}",
            "Buyback {1}",
            "Entwine {1}",
            "Flashback {1}",
            "Madness {1}",
            "Dredge 2",
            "Morph {1}",
            "Megamorph {1}",
            "Disguise {1}",
            "Suspend 2\u{2014}{1}",
            "Evoke {1}",
            "Overload {1}",
            "Living weapon",
            "Enchant player",
            "Changeling",
            "Devoid",
            "Infect",
            "Myriad",
            "Crew 2",
            "Saddle 2",
            "Level up {1}{U}",
            "Outlast {W}",
            "Saddled",
            "Station",
            "Mutate {1}",
            "Encore {1}",
            "Squad {1}",
            "Echo {1}",
            "Cumulative upkeep {1}",
            "Undying",
            "Persist",
            "Exalted",
            "Melee",
            "Mentor",
            "Dethrone",
            "Umbra armor",
            "Annihilator 2",
            "Extort",
            "Evolve",
            "Vanishing 3",
            "Fading 5",
            "Backup 1",
            "Devour 2",
            "Casualty 2",
            "Offspring {2}",
            "Ravenous",
            "Riot",
            "Foretell {1}",
            "Fabricate 1",
            "Ascend",
            "Daybound",
            "Nightbound",
            "Wither",
            "Toxic 2",
            "Fear",
            "Intimidate",
            "Swampwalk",
            "Skulk",
            "Shadow",
            "Split second",
            "Undaunted",
            "Partner with Someone",
            "Partner\u{2014}Friends forever",
            "Doctor's companion",
            "Choose a Background",
            "Unearth {1}",
            "Escape {1}",
            "Rebound",
            "Aftermath",
            "Jump-start",
            "Retrace",
            "Embalm {1}",
            "Eternalize {1}",
            "Scavenge {1}",
            "Transmute {1}{U}{U}",
            "Awaken 3\u{2014}{4}{U}",
            "Gravestorm",
            "Sunburst",
            "Dash {1}",
            "Miracle {1}{W}",
            "Bestow {1}",
            "Blitz {1}",
            "Prowl {1}",
            "Mayhem {1}",
            "Afterlife 2",
            "Horsemanship",
            "Spectacle {B}",
            "Demonstrate",
            "Hideaway 4",
            "Gift a card",
            "For Mirrodin!",
            "Job select",
            "Equip legendary creature {1}",
            "Ninjutsu {1}{U}",
            "Improvise",
            "Affinity for artifacts",
            "Cipher",
            "Graft 2",
            "Afflict 2",
            "Replicate {1}{R}",
            "Plainscycling {2}",
            "Companion \u{2014} Each nonland card in your starting deck has a different name.",
        ];
        let mut seen = Vec::new();
        for sample in samples {
            let keyword = read(sample).unwrap_or_else(|| panic!("{sample} is not read"));
            assert!(
                matches!(keyword.name(), "Landwalk" | "Typecycling" | "Equip to")
                    || sample
                        .to_ascii_lowercase()
                        .starts_with(&printed(keyword.name()).to_ascii_lowercase()),
                "{sample} read as {}",
                keyword.name()
            );
            seen.push(keyword.name());
        }
        let missing: Vec<_> = Keyword::NAMES
            .iter()
            .filter(|name| !seen.contains(name))
            .collect();
        assert!(missing.is_empty(), "{missing:?}");
    }
}
