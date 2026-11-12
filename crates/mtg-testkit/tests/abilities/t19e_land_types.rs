//! Goal: the mana of a land type an effect gives or sets is offered, made and paid with at tables of two and four seats as the mana of a printed type is: Urborg and Yavimaya (corpus cards) and Blood Moon and Magus of the Moon give a land the ability of each of its types once and take the abilities of its text and its old types away, a land taps once for one of them, a spell is paid with it and only with it, and a land is as printed when an effect of a spell ends. (t19e)
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::assemble::assemble;
use mtg_cards::support::{PrintedCard, PrintedFace, Support};
use mtg_core::card::{CardType, Subtype};
use mtg_core::mana::{
    ManaCost,
    ManaType::{self, Black, Blue, Green, Red},
};
use mtg_engine::Event;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::mana::Payment;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

const MOON: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Land"},{"Not":{"Supertype":"Basic"}}]},"mods":[{"SetLandTypes":["Mountain"]}]}}"#;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// The permanent `name` of the tables: a corpus card, a dual land as the server assembles it, or a card that sets the land types.
fn card(name: &str) -> Arc<CardDefinition> {
    match name {
        "Volcanic Island" => {
            let printed = PrintedCard {
                oracle_id: "00000000-0000-0000-0000-000000000019".to_owned(),
                name: name.to_owned(),
                faces: vec![PrintedFace {
                    name: name.to_owned(),
                    type_line: "Land \u{2014} Island Mountain".to_owned(),
                    oracle_text: "({T}: Add {U} or {R}.)".to_owned(),
                    ..Default::default()
                }],
                ..Default::default()
            };
            let assembled = assemble(&printed, None, &mtg_engine::caps());
            assert_eq!(assembled.support.support, Support::Verified);
            Arc::new(assembled.definition)
        }
        "Blood Moon" => Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Enchantment)
                .with_rules(rules(MOON)),
        ),
        "Magus of the Moon" => Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_power_toughness(2, 2)
                .with_rules(rules(MOON)),
        ),
        "Black Spell" | "Blue Spell" | "Red Spell" => {
            let cost = match name {
                "Black Spell" => "{B}",
                "Blue Spell" => "{U}",
                _ => "{R}",
            };
            Arc::new(
                CardDefinition::new(name)
                    .with_type(CardType::Sorcery)
                    .with_mana_cost(ManaCost::parse(cost).unwrap()),
            )
        }
        other => corpus::card(other).unwrap(),
    }
}

const URBORG: &str = "Urborg, Tomb of Yawgmoth";
const YAVIMAYA: &str = "Yavimaya, Cradle of Growth";

/// Seat 0 has the permanents named and the spells named in their hand, at the first main step of turn 1.
fn table(seats: usize, permanents: &[&str], hand: &[&str]) -> Driver {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, permanents.iter().map(|name| ready(card(name))))
        .hand(0, hand.iter().map(|name| card(name)));
    Driver::from_scenario(scenario).unwrap()
}

/// The types of mana the named permanent offers to the player holding priority, one entry for each way, in order.
fn offered(driver: &Driver, name: &str) -> Vec<ManaType> {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    let mut found: Vec<ManaType> = options
        .mana_abilities
        .iter()
        .filter(|m| name_of(driver.state(), m.source.object) == name)
        .map(|m| m.produces)
        .collect();
    found.sort();
    found
}

fn sorted(mana: &[ManaType]) -> Vec<ManaType> {
    let mut list = mana.to_vec();
    list.sort();
    list
}

#[rstest]
#[case::basic_land_with_urborg("Forest", &[URBORG], &[Black, Green])]
#[case::basic_land_with_yavimaya("Forest", &[YAVIMAYA], &[Green])]
#[case::basic_land_under_blood_moon("Forest", &["Blood Moon", URBORG], &[Green])]
#[case::dual_with_urborg("Volcanic Island", &[URBORG], &[Blue, Black, Red])]
#[case::dual_with_urborg_and_yavimaya("Volcanic Island", &[YAVIMAYA, URBORG], &[Blue, Black, Red, Green])]
#[case::dual_under_blood_moon("Volcanic Island", &["Blood Moon"], &[Red])]
#[case::dual_under_magus_of_the_moon("Volcanic Island", &["Magus of the Moon"], &[Red])]
#[case::dual_under_blood_moon_and_urborg("Volcanic Island", &[URBORG, "Blood Moon"], &[Red])]
#[case::dual_under_urborg_and_blood_moon("Volcanic Island", &["Blood Moon", URBORG], &[Red])]
fn a_land_offers_the_mana_of_the_land_types_it_has(
    #[values(2, 4)] seats: usize,
    #[case] land: &str,
    #[case] effects: &[&str],
    #[case] mana: &[ManaType],
) {
    let permanents: Vec<&str> = std::iter::once(land)
        .chain(effects.iter().copied())
        .collect();
    let driver = table(seats, &permanents, &[]);
    assert_eq!(
        offered(&driver, land),
        sorted(mana),
        "{land} with {effects:?}"
    );
}

#[rstest]
#[case::black(Black, &[Black])]
#[case::green(Green, &[Green])]
fn a_land_taps_once_for_the_mana_of_a_type_an_effect_gave_it(
    #[values(2, 4)] seats: usize,
    #[case] wanted: ManaType,
    #[case] added: &[ManaType],
) {
    let mut driver = table(seats, &["Forest", URBORG], &[]);
    let events = driver.tap_for_mana_of("Forest", wanted).unwrap();
    let made: Vec<ManaType> = events
        .iter()
        .filter_map(|event| match event {
            Event::ManaAdded { mana, source, .. }
                if source.is_some_and(|s| s == driver.id_of("Forest").unwrap()) =>
            {
                Some(*mana)
            }
            _ => None,
        })
        .collect();
    assert_eq!(made, added);
    assert!(
        offered(&driver, "Forest").is_empty(),
        "a tapped land offers nothing"
    );
    assert!(driver.tap_for_mana_of("Forest", Black).is_err());
    assert!(driver.tap_for_mana_of("Forest", Green).is_err());
}

/// Starts casting `spell` and pays for it by tapping `land` for `mana` alone.
fn cast_paying_with(
    driver: &mut Driver,
    spell: &str,
    land: &str,
    mana: ManaType,
) -> Result<Vec<Event>, ScriptError> {
    driver.begin_cast(spell)?;
    let DecisionKind::PayMana(options) = &driver.decision()?.kind else {
        panic!("payment was expected");
    };
    let source = options
        .sources
        .iter()
        .find(|o| o.produces == mana && name_of(driver.state(), o.source.object) == land)
        .map(|o| o.source)
        .ok_or_else(|| ScriptError::NoSuchCard {
            name: land.to_owned(),
            place: "payment sources",
        })?;
    let payment = Payment {
        taps: vec![source],
        ..Payment::default()
    };
    driver.answer(Answer::PayMana(PayChoice::Manual(payment)))
}

#[rstest]
#[case::urborg_pays_black("Forest", &[URBORG], "Black Spell", Black, true)]
#[case::the_basic_type_alone_cannot("Forest", &[], "Black Spell", Black, false)]
#[case::yavimaya_gives_no_black("Forest", &[YAVIMAYA], "Black Spell", Black, false)]
#[case::blood_moon_pays_red("Volcanic Island", &["Blood Moon"], "Red Spell", Red, true)]
#[case::blood_moon_takes_blue("Volcanic Island", &["Blood Moon"], "Blue Spell", Blue, false)]
#[case::the_dual_pays_blue_without_it("Volcanic Island", &[], "Blue Spell", Blue, true)]
#[case::urborg_gives_the_dual_black("Volcanic Island", &[URBORG], "Black Spell", Black, true)]
#[case::the_dual_keeps_blue_under_urborg("Volcanic Island", &[URBORG], "Blue Spell", Blue, true)]
#[case::the_mana_must_be_the_one_the_spell_costs("Volcanic Island", &[URBORG], "Blue Spell", Black, false)]
fn a_spell_is_paid_with_the_mana_of_the_land_types_a_land_has_now(
    #[values(2, 4)] seats: usize,
    #[case] land: &str,
    #[case] effects: &[&str],
    #[case] spell: &str,
    #[case] mana: ManaType,
    #[case] paid: bool,
) {
    let permanents: Vec<&str> = std::iter::once(land)
        .chain(effects.iter().copied())
        .collect();
    let mut driver = table(seats, &permanents, &[spell]);
    let cast = cast_paying_with(&mut driver, spell, land, mana);
    assert_eq!(cast.is_ok(), paid, "{spell} with {land} and {effects:?}");
    if paid {
        assert_eq!(driver.state().stack().len(), 1, "the spell is on the stack");
        let id = driver.id_of(land).unwrap();
        let permanent = driver.state().object(id).unwrap().permanent().unwrap();
        assert!(permanent.tapped(), "the land paid");
    }
}
#[rstest]
#[case::gives(Modification::AddSubtype(Subtype("Swamp".to_owned())), &[Blue, Black, Red])]
#[case::sets(Modification::SetLandTypes(vec![Subtype("Mountain".to_owned())]), &[Red])]
fn a_land_is_as_printed_once_an_effect_of_a_spell_has_ended(
    #[values(2, 4)] seats: usize,
    #[case] modification: Modification,
    #[case] during: &[ManaType],
) {
    let mut driver = table(seats, &["Volcanic Island"], &[]);
    let id = driver.id_of("Volcanic Island").unwrap();
    driver
        .add_effect(driver.on(id), modification, Duration::UntilEndOfTurn)
        .unwrap();
    assert_eq!(offered(&driver, "Volcanic Island"), sorted(during));
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Main1).unwrap();
    assert_eq!(offered(&driver, "Volcanic Island"), sorted(&[Blue, Red]));
}
