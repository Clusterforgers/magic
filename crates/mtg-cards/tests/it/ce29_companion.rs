//! Goal: the companion conditions that ask more than a selector of the card (a mana value parity, an activated ability, one card type in common, a mana symbol at most once) read from their printed wording and hold for a deck exactly when every card they speak of fits, and a printed card has an activated ability exactly when a cost line, an activated keyword or a basic land type gives it one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::companion::{CardFact, DeckCard, DeckCondition};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::support::{PrintedCard, PrintedFace};
use mtg_cards::{CardDefinition, has_activated_ability};
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use rstest::rstest;

fn nonland() -> Selector {
    Selector(vec![Pred::Not(Box::new(Pred::Type(CardType::Land)))])
}

fn permanent() -> Selector {
    Selector(vec![Pred::AnyOf(
        [
            CardType::Artifact,
            CardType::Creature,
            CardType::Enchantment,
            CardType::Land,
            CardType::Planeswalker,
            CardType::Battle,
        ]
        .map(Pred::Type)
        .to_vec(),
    )])
}

fn gyruda() -> DeckCondition {
    DeckCondition::Has {
        of: Selector::default(),
        fact: CardFact::EvenManaValue,
    }
}

fn obosh() -> DeckCondition {
    DeckCondition::Has {
        of: nonland(),
        fact: CardFact::OddManaValue,
    }
}

fn zirda() -> DeckCondition {
    DeckCondition::Has {
        of: permanent(),
        fact: CardFact::ActivatedAbility,
    }
}

fn jegantha() -> DeckCondition {
    DeckCondition::Has {
        of: Selector::default(),
        fact: CardFact::NoRepeatedSymbol,
    }
}

fn umori() -> DeckCondition {
    DeckCondition::SharedType { of: nonland() }
}

#[rstest]
#[case::gyruda(
    "Your starting deck contains only cards with even mana values.",
    gyruda()
)]
#[case::obosh(
    "Your starting deck contains only cards with odd mana values and land cards.",
    obosh()
)]
#[case::zirda(
    "Each permanent card in your starting deck has an activated ability.",
    zirda()
)]
#[case::jegantha(
    "No card in your starting deck has more than one of the same mana symbol in its mana cost.",
    jegantha()
)]
#[case::umori("Each nonland card in your starting deck shares a card type.", umori())]
fn the_wordings_read(#[case] text: &str, #[case] condition: DeckCondition) {
    let line = format!("Companion \u{2014} {text}");
    assert_eq!(
        mtg_cards::compile::read_keyword(&line),
        Some(Keyword::companion(condition.clone()))
    );
    assert!(condition.unreadable().is_empty());
    let json = serde_json::to_string(&condition).unwrap();
    assert_eq!(
        serde_json::from_str::<DeckCondition>(&json).unwrap(),
        condition
    );
}

/// A card of the deck: its name, cost (`{X}` counted apart, as the definition keeps it), card types and whether it has
/// an activated ability.
fn card(name: &str, cost: &str, types: &[CardType], activated: bool) -> DeckCard {
    let mut definition = CardDefinition::new(name);
    for ty in types {
        definition = definition.with_type(*ty);
    }
    let xs = cost.matches("{X}").count();
    let rest = cost.replace("{X}", "");
    if !rest.is_empty() {
        definition = definition.with_mana_cost(ManaCost::parse(&rest).unwrap());
    }
    definition.x_in_cost = u8::try_from(xs).unwrap();
    definition.activated = activated;
    DeckCard::from(&definition)
}

const C: CardType = CardType::Creature;
const L: CardType = CardType::Land;
const I: CardType = CardType::Instant;
const A: CardType = CardType::Artifact;
const S: CardType = CardType::Sorcery;

#[rstest]
#[case::gyruda_even_costs_and_lands(gyruda(), vec![card("a", "{1}{U}", &[C], false), card("b", "", &[L], false), card("c", "{2}{B}{B}", &[I], false)], true)]
#[case::gyruda_an_odd_cost(gyruda(), vec![card("a", "{1}{U}", &[C], false), card("b", "{U}", &[I], false)], false)]
#[case::gyruda_x_counts_zero(gyruda(), vec![card("a", "{X}{U}", &[S], false)], false)]
#[case::obosh_odd_costs_and_lands(obosh(), vec![card("a", "{R}", &[C], false), card("b", "", &[L], false), card("c", "{1}{B}{R}", &[S], false)], true)]
#[case::obosh_an_even_cost(obosh(), vec![card("a", "{R}", &[C], false), card("b", "{1}{R}", &[I], false)], false)]
#[case::zirda_permanents_with_abilities(zirda(), vec![card("a", "{1}{W}", &[C], true), card("b", "{R}", &[I], false)], true)]
#[case::zirda_a_vanilla_creature(zirda(), vec![card("a", "{1}{W}", &[C], true), card("b", "{1}{G}", &[C], false)], false)]
#[case::jegantha_distinct_symbols(jegantha(), vec![card("a", "{2}{R}{G}", &[C], false), card("b", "", &[L], false)], true)]
#[case::jegantha_a_doubled_color(jegantha(), vec![card("a", "{1}{R}{R}", &[C], false)], false)]
#[case::jegantha_two_x(jegantha(), vec![card("a", "{X}{X}{R}", &[S], false)], false)]
#[case::jegantha_one_x(jegantha(), vec![card("a", "{X}{R}", &[S], false)], true)]
#[case::umori_all_creatures(umori(), vec![card("a", "{G}", &[C], false), card("b", "{1}{G}", &[C, A], false), card("c", "", &[L], false)], true)]
#[case::umori_an_instant(umori(), vec![card("a", "{G}", &[C], false), card("b", "{G}", &[I], false)], false)]
#[case::umori_the_common_type_narrows(umori(), vec![card("a", "{G}", &[C, A], false), card("b", "{G}", &[A], false), card("c", "{G}", &[C], false)], false)]
#[case::umori_an_empty_deck(umori(), vec![], true)]
fn the_conditions_hold_over_decks(
    #[case] condition: DeckCondition,
    #[case] deck: Vec<DeckCard>,
    #[case] holds: bool,
) {
    let result = condition.check(&deck, 60);
    assert_eq!(result.is_ok(), holds, "{result:?}");
}

fn printed(layout: &str, keywords: &[&str], faces: &[(&str, &str)]) -> PrintedCard {
    PrintedCard {
        oracle_id: String::new(),
        name: "Probe".into(),
        keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
        layout: layout.into(),
        faces: faces
            .iter()
            .map(|(type_line, text)| PrintedFace {
                name: "Probe".into(),
                type_line: (*type_line).into(),
                oracle_text: (*text).into(),
                ..PrintedFace::default()
            })
            .collect(),
    }
}

#[rstest]
#[case::a_cost_line(printed("normal", &[], &[("Creature", "{1}, {T}: Target creature can't block this turn.")]), true)]
#[case::a_loyalty_ability(printed("normal", &[], &[("Legendary Planeswalker", "+1: Draw a card.")]), true)]
#[case::equip(printed("normal", &["Equip"], &[("Artifact \u{2014} Equipment", "Equip {2}")]), true)]
#[case::landcycling(printed("normal", &["Plainscycling"], &[("Creature", "Plainscycling {2}")]), true)]
#[case::a_basic_land_type(printed("normal", &[], &[("Land \u{2014} Forest Island", "")]), true)]
#[case::vanilla(printed("normal", &[], &[("Creature \u{2014} Bear", "")]), false)]
#[case::a_trigger_with_a_colon(printed("normal", &[], &[("Creature", "When this creature enters, choose one: draw a card.")]), false)]
#[case::a_granted_ability(printed("normal", &[], &[("Creature", "Creatures you control have \"{T}: Add {G}.\"")]), false)]
#[case::reminder_text_only(printed("normal", &[], &[("Creature", "Vigilance (Attacking doesn't cause this creature to tap: it stays untapped.)")]), false)]
#[case::the_back_face_of_a_dfc(printed("transform", &[], &[("Creature", "Flying"), ("Creature", "{T}: Draw a card.")]), false)]
#[case::a_split_card_half(printed("split", &[], &[("Instant", "Draw a card."), ("Enchantment", "{1}: Scry 1.")]), true)]
fn a_printed_card_has_an_activated_ability(#[case] card: PrintedCard, #[case] has: bool) {
    assert_eq!(has_activated_ability(&card), has);
}
