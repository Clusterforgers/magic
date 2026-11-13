//! The wordings of a choice of a type made as a permanent enters: a card type, one of a list of card types, or a basic
//! land type ("As ~ enters, choose artifact, creature, enchantment, instant, or sorcery").

use mtg_core::card::{CardType, CardTypes};

use super::tokens::Reader;
use crate::trigger::EntersChoice;
use crate::value::{Amount, CounterKind};

/// Every card type, in the order the rules list them (CR 205.2a).
const CARD_TYPES: [CardType; 9] = [
    CardType::Artifact,
    CardType::Battle,
    CardType::Creature,
    CardType::Enchantment,
    CardType::Instant,
    CardType::Kindred,
    CardType::Land,
    CardType::Planeswalker,
    CardType::Sorcery,
];

/// "As ~ enters, choose ..." for a choice of a card type or a basic land type; empty for another choice.
pub(super) fn choose_type_texts(kind: EntersChoice) -> Vec<String> {
    let what = match kind {
        EntersChoice::CardType => vec!["a card type".to_owned()],
        EntersChoice::BasicLandType => vec!["a basic land type".to_owned()],
        EntersChoice::CardTypeOf(set) => type_list_texts(set),
        _ => return Vec::new(),
    };
    what.into_iter()
        .map(|what| format!("As ~ enters, choose {what}"))
        .collect()
}

/// "artifact, creature, enchantment, instant, or sorcery" (the types named) and "a card type other than creature or
/// land" (the types left out), for a set of at least two types.
fn type_list_texts(set: CardTypes) -> Vec<String> {
    let (named, left): (Vec<CardType>, Vec<CardType>) =
        CARD_TYPES.iter().partition(|kind| set.contains(**kind));
    if named.len() < 2 || left.is_empty() {
        return Vec::new();
    }
    // The rules order, and that order with one type moved last ("artifact, enchantment, instant, sorcery, or
    // planeswalker"): one wording per type, never every order.
    let mut texts = vec![or_list(&named)];
    for at in 0..named.len() - 1 {
        let mut moved = named.clone();
        let last = moved.remove(at);
        moved.push(last);
        texts.push(or_list(&moved));
    }
    if left.len() <= 2 {
        texts.push(format!("a card type other than {}", or_list(&left)));
    }
    texts
}

/// "~ enters with your choice of a deathtouch counter or a lifelink counter on it"; empty for fewer than two kinds.
pub(super) fn counter_of_texts(kinds: &[CounterKind]) -> Vec<String> {
    let Some(each) = kinds
        .iter()
        .map(|kind| super::counters::text(kind, &Amount::one()))
        .collect::<Option<Vec<String>>>()
    else {
        return Vec::new();
    };
    // A choice between two is the same in either order.
    let lists = match each.as_slice() {
        [one, two] => vec![format!("{one} or {two}"), format!("{two} or {one}")],
        [rest @ .., last] if !rest.is_empty() => vec![format!("{}, or {last}", rest.join(", "))],
        _ => return Vec::new(),
    };
    lists
        .into_iter()
        .map(|list| format!("~ enters with your choice of {list} on it"))
        .collect()
}

/// "Spells you cast of the chosen type" beside "Spells of the chosen type you cast", and "Spells your opponents cast
/// with the chosen name" beside "Spells with the chosen name your opponents cast".
pub(super) fn cast_before_chosen(subject: &str) -> Option<String> {
    ["of the chosen type", "with the chosen name"]
        .iter()
        .find_map(|chosen| {
            ["you cast", "your opponents cast"].iter().find_map(|cast| {
                let head = subject.strip_suffix(&format!(" {chosen} {cast}"))?;
                Some(format!("{head} {cast} {chosen}"))
            })
        })
}

/// "artifact, creature, or sorcery"; "creature or land" for two.
fn or_list(kinds: &[CardType]) -> String {
    let words: Vec<String> = kinds
        .iter()
        .map(|kind| format!("{kind:?}").to_lowercase())
        .collect();
    match words.as_slice() {
        [one, two] => format!("{one} or {two}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
        [] => String::new(),
    }
}

/// The choice after "As ~ enters, choose": a card type, a basic land type or a list of card types; `None` for other
/// words, which leave the reader where it was.
pub(super) fn read_choose_type(r: &mut Reader<'_>) -> Option<EntersChoice> {
    if r.words("a card type") {
        return Some(EntersChoice::CardType);
    }
    if r.words("a basic land type") {
        return Some(EntersChoice::BasicLandType);
    }
    None
}

#[cfg(test)]
mod tests {
    //! Goal: each choice of a type reads back only as the list it offers.
    use super::*;
    use rstest::rstest;

    fn set(kinds: &[CardType]) -> CardTypes {
        kinds.iter().copied().collect()
    }

    #[rstest]
    #[case::any_card_type(EntersChoice::CardType, "As ~ enters, choose a card type")]
    #[case::basic_land_type(EntersChoice::BasicLandType, "As ~ enters, choose a basic land type")]
    #[case::five_named(
        EntersChoice::CardTypeOf(set(&[CardType::Artifact, CardType::Creature, CardType::Enchantment, CardType::Instant, CardType::Sorcery])),
        "As ~ enters, choose artifact, creature, enchantment, instant, or sorcery"
    )]
    #[case::one_moved_last(
        EntersChoice::CardTypeOf(set(&[CardType::Artifact, CardType::Enchantment, CardType::Instant, CardType::Planeswalker, CardType::Sorcery])),
        "As ~ enters, choose artifact, enchantment, instant, sorcery, or planeswalker"
    )]
    #[case::two_left_out(
        EntersChoice::CardTypeOf(set(&[CardType::Artifact, CardType::Battle, CardType::Enchantment, CardType::Instant, CardType::Kindred, CardType::Planeswalker, CardType::Sorcery])),
        "As ~ enters, choose a card type other than creature or land"
    )]
    fn says(#[case] kind: EntersChoice, #[case] text: &str) {
        assert!(choose_type_texts(kind).iter().any(|said| said == text));
    }

    #[rstest]
    #[case::a_land_type_is_no_basic_one(
        EntersChoice::LandType,
        "As ~ enters, choose a basic land type"
    )]
    #[case::a_creature_type(EntersChoice::CreatureType, "As ~ enters, choose a card type")]
    #[case::other_list(
        EntersChoice::CardTypeOf(set(&[CardType::Artifact, CardType::Creature, CardType::Enchantment, CardType::Instant, CardType::Planeswalker])),
        "As ~ enters, choose artifact, creature, enchantment, instant, or sorcery"
    )]
    #[case::one_more_left_out(
        EntersChoice::CardTypeOf(set(&[CardType::Artifact, CardType::Enchantment, CardType::Instant, CardType::Kindred, CardType::Planeswalker, CardType::Sorcery])),
        "As ~ enters, choose a card type other than creature or land"
    )]
    fn differs(#[case] kind: EntersChoice, #[case] text: &str) {
        assert!(choose_type_texts(kind).iter().all(|said| said != text));
    }

    #[rstest]
    #[case::moved(
        "Spells of the chosen type you cast",
        Some("Spells you cast of the chosen type")
    )]
    #[case::colour("Spells of the chosen color you cast", None)]
    #[case::opponents(
        "Spells of the chosen type your opponents cast",
        Some("Spells your opponents cast of the chosen type")
    )]
    #[case::named(
        "Spells with the chosen name you cast",
        Some("Spells you cast with the chosen name")
    )]
    #[case::not_cast("Spells with the chosen name", None)]
    fn cast_before(#[case] subject: &str, #[case] moved: Option<&str>) {
        assert_eq!(cast_before_chosen(subject).as_deref(), moved);
    }

    fn kinds(names: &[&str]) -> Vec<CounterKind> {
        names
            .iter()
            .map(|name| CounterKind::Other((*name).to_owned()))
            .collect()
    }

    const TWO: &str =
        "~ enters with your choice of a deathtouch counter or a lifelink counter on it";

    #[rstest]
    #[case::two(&["deathtouch", "lifelink"], TWO, true)]
    #[case::three(
        &["menace", "deathtouch", "lifelink"],
        "~ enters with your choice of a menace counter, a deathtouch counter, or a lifelink counter on it",
        true
    )]
    #[case::other_kind(&["deathtouch", "menace"], TWO, false)]
    #[case::either_order_of_two(&["lifelink", "deathtouch"], TWO, true)]
    #[case::one_kind_only(&["deathtouch"], "~ enters with a deathtouch counter on it", false)]
    fn a_counter_of_a_chosen_kind(#[case] names: &[&str], #[case] text: &str, #[case] read: bool) {
        let said = counter_of_texts(&kinds(names));
        assert_eq!(said.iter().any(|said| said == text), read, "{said:?}");
    }
}
