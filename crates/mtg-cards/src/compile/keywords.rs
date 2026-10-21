//! Stage 4: keyword lines. Which words are keywords, and the keywords the DSL declares read with their parameters.

use std::ops::Range;
use std::str::FromStr;

use mtg_core::card::CardType;
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
    ("partner", Params::Dash),
    ("companion", Params::Dash),
    ("ward", Params::Cost),
    ("equip", Params::Cost),
    ("reconfigure", Params::Cost),
    ("cycling", Params::Cost),
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
        "kicker" => cost(rest).map(Keyword::Kicker),
        "multikicker" => cost(rest).map(Keyword::Multikicker),
        "flashback" => cost(rest).map(Keyword::Flashback),
        "madness" => cost(rest).map(Keyword::Madness),
        "morph" => cost(rest).map(Keyword::Morph),
        "disguise" => cost(rest).map(Keyword::Disguise),
        "evoke" => cost(rest).map(Keyword::Evoke),
        "overload" => cost(rest).map(Keyword::Overload),
        "suspend" => suspend(rest),
        "enchant" => enchant(rest).map(Keyword::Enchant),
        "partner with" => Some(Keyword::PartnerWith(rest.to_owned())),
        "protection" => protection(rest).map(Keyword::Protection),
        "companion" => Some(Keyword::Companion),
        name if rest.is_empty() => plain(name),
        _ => None,
    }
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
        "delve" => Keyword::Delve,
        "cascade" => Keyword::Cascade,
        "storm" => Keyword::Storm,
        "partner" => Keyword::Partner,
        "battle cry" => Keyword::BattleCry,
        "living weapon" => Keyword::LivingWeapon,
        "fuse" => Keyword::Fuse,
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
    let pred = match word.as_str() {
        "everything" => return Some(Selector::default()),
        "white" => Pred::Color(Color::White),
        "blue" => Pred::Color(Color::Blue),
        "black" => Pred::Color(Color::Black),
        "red" => Pred::Color(Color::Red),
        "green" => Pred::Color(Color::Green),
        "colorless" => Pred::Colorless,
        "creatures" => Pred::Type(CardType::Creature),
        "artifacts" => Pred::Type(CardType::Artifact),
        "enchantments" => Pred::Type(CardType::Enchantment),
        "planeswalkers" => Pred::Type(CardType::Planeswalker),
        "lands" => Pred::Type(CardType::Land),
        _ => return None,
    };
    Some(Selector(vec![pred]))
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

/// `4—{U}`: the time counters and the mana cost.
fn suspend(rest: &str) -> Option<Keyword> {
    let (time, price) = rest.split_once('\u{2014}')?;
    Some(Keyword::Suspend {
        time: time.trim().parse().ok()?,
        cost: cost(price.trim())?,
    })
}

/// What an Aura enchants, when it is one card type.
fn enchant(rest: &str) -> Option<Selector> {
    let card_type = match rest.to_ascii_lowercase().as_str() {
        "creature" => CardType::Creature,
        "land" => CardType::Land,
        "artifact" => CardType::Artifact,
        "enchantment" => CardType::Enchantment,
        "planeswalker" => CardType::Planeswalker,
        _ => return None,
    };
    Some(Selector(vec![Pred::Type(card_type)]))
}

#[cfg(test)]
mod tests {
    //! Goal: keyword lines are recognised in every wording the cards print, the keywords the DSL declares read with their parameters, and anything else stays unread.
    use super::*;
    use rstest::rstest;

    fn mana(text: &str) -> Cost {
        Cost::mana(text.parse().unwrap())
    }

    fn creature() -> Selector {
        Selector(vec![Pred::Type(CardType::Creature)])
    }

    #[rstest]
    #[case::flying("Flying", Keyword::Flying)]
    #[case::lowercase("flying", Keyword::Flying)]
    #[case::two_words("First strike", Keyword::FirstStrike)]
    #[case::lowercase_two_words("double strike", Keyword::DoubleStrike)]
    #[case::battle_cry("Battle cry", Keyword::BattleCry)]
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
    #[case::equip("Equip {1}", Keyword::Equip(mana("{1}")))]
    #[case::equip_two("Equip {2}{G}", Keyword::Equip(mana("{2}{G}")))]
    #[case::kicker("Kicker {4}", Keyword::Kicker(mana("{4}")))]
    #[case::multikicker("Multikicker {1}{G}", Keyword::Multikicker(mana("{1}{G}")))]
    #[case::flashback("Flashback {2}{R}", Keyword::Flashback(mana("{2}{R}")))]
    #[case::evoke("Evoke {2}{U}", Keyword::Evoke(mana("{2}{U}")))]
    #[case::cycling("Cycling {2}", Keyword::Cycling(mana("{2}")))]
    #[case::madness("Madness {R}", Keyword::Madness(mana("{R}")))]
    #[case::morph("Morph {3}{B}", Keyword::Morph(mana("{3}{B}")))]
    #[case::disguise("Disguise {1}{W}", Keyword::Disguise(mana("{1}{W}")))]
    #[case::overload("Overload {3}{U}{U}", Keyword::Overload(mana("{3}{U}{U}")))]
    #[case::reconfigure("Reconfigure {2}", Keyword::Reconfigure(mana("{2}")))]
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
    #[case::companion(
        "Companion \u{2014} Each nonland card in your starting deck has a different name.",
        Keyword::Companion
    )]
    #[case::partner_with(
        "Partner with Pir, Imaginative Rascal",
        Keyword::PartnerWith("Pir, Imaginative Rascal".into())
    )]
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
    #[case::enchant_player("Enchant player")]
    #[case::enchant_with_a_condition("Enchant creature you control")]
    #[case::enchant_two_types("Enchant creature or planeswalker")]
    #[case::partner_friends("Partner\u{2014}Friends forever")]
    #[case::undeclared("Undying")]
    #[case::level_up("Level up {1}{U}")]
    #[case::typecycling("Plainscycling {2}")]
    #[case::landwalk("Islandwalk")]
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
    #[case::undeclared_in_a_list("Flying, undying", &["Flying", "undying"])]
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

    #[test]
    fn every_keyword_the_dsl_declares_is_recognised_by_name() {
        for name in Keyword::NAMES {
            assert!(
                KEYWORDS
                    .iter()
                    .any(|(known, _)| known.eq_ignore_ascii_case(name)),
                "{name} is missing from the keyword table"
            );
        }
    }

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
            "Flashback {1}",
            "Madness {1}",
            "Morph {1}",
            "Disguise {1}",
            "Suspend 2\u{2014}{1}",
            "Evoke {1}",
            "Overload {1}",
            "Living weapon",
            "Partner with Someone",
            "Companion \u{2014} Each nonland card in your starting deck has a different name.",
        ];
        let mut seen = Vec::new();
        for sample in samples {
            let keyword = read(sample).unwrap_or_else(|| panic!("{sample} is not read"));
            assert!(
                sample
                    .to_ascii_lowercase()
                    .starts_with(&keyword.name().to_ascii_lowercase()),
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
