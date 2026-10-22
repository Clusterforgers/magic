//! Stage 4: keyword lines. Which words are keywords, and the keywords the DSL declares read with their parameters.

use std::ops::Range;
use std::str::FromStr;

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;

use crate::cost::{Cost, CostItem};
use crate::keyword::Keyword;
use crate::mana::ManaCostX;
use crate::refs::{Pred, Selector};
use crate::value::Amount;

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
    ("megamorph", Params::Cost),
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
    ("transmute", Params::Cost),
    ("level up", Params::Cost),
    ("embalm", Params::Cost),
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
    ("modular", Params::Number),
    ("poisonous", Params::Number),
    ("rampage", Params::Number),
    ("renown", Params::Number),
    ("ripple", Params::Number),
    ("soulshift", Params::Number),
    ("toxic", Params::Number),
    ("tribute", Params::Number),
    ("vanishing", Params::Number),
    ("devour", Params::Number),
    ("frenzy", Params::Number),
    ("mobilize", Params::Number),
    ("saddle", Params::Number),
    ("saddled", Params::None),
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
        "equip" => cost(rest).map(Keyword::Equip),
        "reconfigure" => cost(rest).map(Keyword::Reconfigure),
        "cycling" => cost(rest).map(Keyword::Cycling),
        "kicker" => spell_cost(rest).map(Keyword::Kicker),
        "multikicker" => cost(rest).map(Keyword::Multikicker),
        "buyback" => spell_cost(rest).map(Keyword::Buyback),
        "entwine" => spell_cost(rest).map(Keyword::Entwine),
        "flashback" => spell_cost(rest).map(Keyword::Flashback),
        "madness" => spell_cost(rest).map(Keyword::Madness),
        "miracle" => spell_cost(rest).map(Keyword::Miracle),
        "dredge" => rest
            .parse()
            .ok()
            .filter(|cards| *cards > 0)
            .map(Keyword::Dredge),
        "morph" => cost(rest).map(Keyword::Morph),
        "disguise" => cost(rest).map(Keyword::Disguise),
        "evoke" => spell_cost(rest).map(Keyword::Evoke),
        "overload" => cost(rest).map(Keyword::Overload),
        "mutate" => cost(rest).map(Keyword::Mutate),
        "encore" => cost(rest).map(Keyword::Encore),
        "squad" => spell_cost(rest).map(Keyword::Squad),
        "foretell" => cost(rest).map(Keyword::Foretell),
        "unearth" => spell_cost(rest).map(Keyword::Unearth),
        "embalm" => spell_cost(rest).map(Keyword::Embalm),
        "eternalize" => spell_cost(rest).map(Keyword::Eternalize),
        "escape" => spell_cost(rest).map(Keyword::Escape),
        "dash" => spell_cost(rest).map(Keyword::Dash),
        "bestow" => spell_cost(rest).map(Keyword::Bestow),
        "blitz" => spell_cost(rest).map(Keyword::Blitz),
        "ninjutsu" => cost(rest).map(Keyword::Ninjutsu),
        "echo" => spell_cost(rest).map(Keyword::Echo),
        "cumulative upkeep" => spell_cost(rest).map(Keyword::CumulativeUpkeep),
        "vanishing" => rest
            .parse()
            .ok()
            .filter(|time| *time > 0)
            .map(Keyword::Vanishing),
        "fading" => rest
            .parse()
            .ok()
            .filter(|fade| *fade > 0)
            .map(Keyword::Fading),
        "backup" => rest
            .parse()
            .ok()
            .filter(|count| *count > 0)
            .map(|count| Keyword::Backup {
                count,
                abilities: Vec::new(),
            }),
        "crew" => rest
            .parse()
            .ok()
            .filter(|power| *power > 0)
            .map(Keyword::Crew),
        "saddle" => rest
            .parse()
            .ok()
            .filter(|power| *power > 0)
            .map(Keyword::Saddle),
        "fabricate" => rest
            .parse()
            .ok()
            .filter(|number| *number > 0)
            .map(Keyword::Fabricate),
        "level up" => cost(rest).map(Keyword::LevelUp),
        "outlast" => cost(rest).map(Keyword::Outlast),
        "suspend" => suspend(rest),
        "enchant" => super::read::enchant_filter(rest).map(Keyword::Enchant),
        "enchant player" => Some(Keyword::EnchantPlayer { opponent: false }),
        "enchant opponent" => Some(Keyword::EnchantPlayer { opponent: true }),
        "partner with" => Some(Keyword::PartnerWith(rest.to_owned())),
        "partner" if rest.starts_with('\u{2014}') => partner_group(rest),
        "protection" => protection(rest).map(Keyword::Protection),
        "companion" => Some(Keyword::Companion),
        "toxic" => rest
            .parse()
            .ok()
            .filter(|poison| *poison > 0)
            .map(Keyword::Toxic),
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

/// The text of a `Partner—<text>` item (`Friends forever`, `Character select`), the group of commanders that pair.
fn partner_group(rest: &str) -> Option<Keyword> {
    let text = rest
        .trim_start_matches('\u{2014}')
        .trim()
        .trim_end_matches('.')
        .trim();
    (!text.is_empty()).then(|| Keyword::PartnerGroup(text.to_owned()))
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
        "partner" => Keyword::Partner,
        "battle cry" => Keyword::BattleCry,
        "living weapon" => Keyword::LivingWeapon,
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
        "split second" => Keyword::SplitSecond,
        "undaunted" => Keyword::Undaunted,
        "rebound" => Keyword::Rebound,
        "aftermath" => Keyword::Aftermath,
        "undying" => Keyword::Undying,
        "persist" => Keyword::Persist,
        "saddled" => Keyword::Saddled,
        "ravenous" => Keyword::Ravenous,
        "ascend" => Keyword::Ascend,
        "jump-start" => Keyword::JumpStart,
        "retrace" => Keyword::Retrace,
        _ => return None,
    })
}

/// The quality a protection names, when it is one colour, colourless, one kind of permanent or everything:
/// "from red", "from creatures", "from everything". The empty selector matches every source.
fn protection(rest: &str) -> Option<Selector> {
    let word = rest
        .strip_prefix("from ")?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    quality(&word)
}

/// The words of a protection quality, each with the sources it names.
const QUALITIES: [(&str, Pred); 11] = [
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

/// The word of a protection quality, the inverse of `quality`.
pub fn quality_word(quality: &Selector) -> Option<&'static str> {
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

    fn finds(preds: Vec<Pred>, cost: &str) -> Keyword {
        Keyword::Typecycling {
            find: Selector(preds),
            cost: mana(cost),
        }
    }

    fn creature() -> Selector {
        Selector(vec![Pred::Type(CardType::Creature)])
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
    #[case::split_second("Split second", Keyword::SplitSecond)]
    #[case::undaunted("Undaunted", Keyword::Undaunted)]
    #[case::islandwalk("Islandwalk", walks("Island"))]
    #[case::forestwalk("forestwalk", walks("Forest"))]
    #[case::desertwalk("Desertwalk", walks("Desert"))]
    #[case::equip("Equip {1}", Keyword::Equip(mana("{1}")))]
    #[case::equip_two("Equip {2}{G}", Keyword::Equip(mana("{2}{G}")))]
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
    #[case::madness("Madness {R}", Keyword::Madness(mana("{R}")))]
    #[case::miracle("Miracle {W}", Keyword::Miracle(mana("{W}")))]
    #[case::dredge("Dredge 3", Keyword::Dredge(3))]
    #[case::dredge_six("Dredge 6", Keyword::Dredge(6))]
    #[case::morph("Morph {3}{B}", Keyword::Morph(mana("{3}{B}")))]
    #[case::disguise("Disguise {1}{W}", Keyword::Disguise(mana("{1}{W}")))]
    #[case::overload("Overload {3}{U}{U}", Keyword::Overload(mana("{3}{U}{U}")))]
    #[case::reconfigure("Reconfigure {2}", Keyword::Reconfigure(mana("{2}")))]
    #[case::mutate("Mutate {1}{G}{G}", Keyword::Mutate(mana("{1}{G}{G}")))]
    #[case::encore("Encore {5}{R}", Keyword::Encore(mana("{5}{R}")))]
    #[case::squad("Squad {2}", Keyword::Squad(mana("{2}")))]
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
    #[case::ravenous("Ravenous", Keyword::Ravenous)]
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
        Keyword::Companion
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
    #[case::qualified_equip("Equip legendary creature {3}")]
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
    #[case::undeclared("Horsemanship")]
    #[case::crew_x("Crew X")]
    #[case::dredge_x("Dredge X")]
    #[case::dredge_nothing("Dredge 0")]
    #[case::fabricate_x("Fabricate X")]
    #[case::fabricate_nothing("Fabricate 0")]
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
            "Mutate {1}",
            "Encore {1}",
            "Squad {1}",
            "Echo {1}",
            "Cumulative upkeep {1}",
            "Undying",
            "Persist",
            "Vanishing 3",
            "Fading 5",
            "Backup 1",
            "Ravenous",
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
            "Dash {1}",
            "Miracle {1}{W}",
            "Bestow {1}",
            "Blitz {1}",
            "Ninjutsu {1}{U}",
            "Improvise",
            "Affinity for artifacts",
            "Plainscycling {2}",
            "Companion \u{2014} Each nonland card in your starting deck has a different name.",
        ];
        let mut seen = Vec::new();
        for sample in samples {
            let keyword = read(sample).unwrap_or_else(|| panic!("{sample} is not read"));
            assert!(
                matches!(keyword.name(), "Landwalk" | "Typecycling")
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
