//! Goal: at tables of two to four seats the offer says why each card the holder could play is blocked, beyond the hand: a "can't cast more than N spells" rule (the One Voice modifier on the boss's seat, Rule of Law) is named with its limit once it bites, a commander short of mana shows the tax in its cost, a graveyard card with a way to cast (flashback) and the top of the library under Future Sight say why, and a spent spell in a graveyard or a top card no permission covers is never named.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PriorityOptions, Unplayable, Why};
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, island, mountain};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn card(name: &str) -> Arc<CardDefinition> {
    mtg_testkit::corpus::card(name).unwrap()
}

/// The One Voice encounter modifier as the boss's emblem: "Each player can't cast more than one spell each turn."
fn one_voice() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"Rule":{"rule":{"Cant":{"who":"EachPlayer","action":{"Cast":{"more_than":1}}}}}}"#,
    )
    .unwrap();
    Arc::new(emblem_definition("One Voice", &[rule]))
}

/// An enchantment with Future Sight's permission: play lands and cast spells from the top of the library.
fn future_sight() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Future Sight")
            .with_type(CardType::Enchantment)
            .with_rules(
                serde_json::from_str(
                    r#"{"Rule":{"rule":{"MayCast":{"what":["TopOfLibrary"],"from":"Library","play":true}}}}"#,
                )
                .unwrap(),
            ),
    )
}

/// `players` seats with seat 0 active in `step` of turn 2 and ten Islands in every library.
fn seats(players: usize, step: Step) -> Scenario {
    let mut scenario = Scenario::new(players).at(2, 0, step);
    for seat in 0..players {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("not a priority decision: {other:?}"),
    }
}

/// The reason and detail the offer gives for `card`.
fn told(driver: &Driver, card: ObjectId) -> Option<(Unplayable, Option<Why>)> {
    options(driver)
        .unplayable
        .into_iter()
        .find(|entry| entry.card == card)
        .map(|entry| (entry.reason, entry.why))
}

/// The first card named `name` in `zone`.
fn in_zone(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    let state = driver.state();
    *state
        .zone(zone)
        .iter()
        .find(|id| state.object(**id).unwrap().card().name == name)
        .unwrap()
}

fn emblem(driver: &Driver, seat: usize, name: &str) -> ObjectId {
    in_zone(driver, Zone::Command(PlayerId(seat as u64)), name)
}

#[rstest]
fn one_voice_names_the_modifier_and_its_limit_after_the_first_spell(
    #[values(2, 3, 4)] players: usize,
) {
    let boss = players - 1;
    let mut driver = Driver::from_scenario(
        seats(players, Step::Main1)
            .emblems(boss, [one_voice()])
            .hand(0, [card("Opt"), card("Opt")])
            .battlefield(0, [ready(island()), ready(island())]),
    )
    .unwrap();
    let first = in_zone(&driver, Zone::Hand(PlayerId(0)), "Opt");
    assert_eq!(told(&driver, first), None, "the first spell is open");
    driver.cast("Opt").unwrap();
    let second = in_zone(&driver, Zone::Hand(PlayerId(0)), "Opt");
    let source = emblem(&driver, boss, "One Voice");
    assert_eq!(
        told(&driver, second),
        Some((
            Unplayable::Restricted,
            Some(Why::ForbiddenBy {
                source: Some(source),
                name: Some("One Voice".to_owned()),
                limit: Some(1),
            })
        ))
    );
}

#[rstest]
fn rule_of_law_on_the_battlefield_is_named_with_its_limit(#[values(2, 3, 4)] players: usize) {
    let mut driver = Driver::from_scenario(
        seats(players, Step::Main1)
            .hand(0, [card("Opt"), card("Opt")])
            .battlefield(0, [ready(island()), ready(island())])
            .battlefield(1, [ready(card("Rule of Law"))]),
    )
    .unwrap();
    driver.cast("Opt").unwrap();
    let second = in_zone(&driver, Zone::Hand(PlayerId(0)), "Opt");
    let law = driver.id_of("Rule of Law").unwrap();
    assert_eq!(
        told(&driver, second),
        Some((
            Unplayable::Restricted,
            Some(Why::ForbiddenBy {
                source: Some(law),
                name: Some("Rule of Law".to_owned()),
                limit: Some(1),
            })
        ))
    );
}

#[rstest]
fn a_commander_short_of_mana_shows_the_tax_in_its_cost(#[values(2, 3, 4)] players: usize) {
    let mut driver = Driver::from_scenario(
        seats(players, Step::Main1)
            .format(Arc::new(Commander))
            .commanders(0, [legend("Cmd", 1, &[Color::Red], 2, 2)])
            .battlefield(0, (0..3).map(|_| ready(mountain()))),
    )
    .unwrap();
    let id = driver.state().commanders().next().unwrap();
    assert_eq!(told(&driver, id), None, "the first cast is open");
    driver.cast("Cmd").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    driver
        .manual(ManualAction::Move {
            object: id,
            to: ManualZone::Graveyard,
        })
        .unwrap();
    driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
    assert_eq!(
        driver.state().object(id).unwrap().zone(),
        Zone::Command(PlayerId(0))
    );
    assert_eq!(
        told(&driver, id),
        Some((
            Unplayable::Mana,
            Some(Why::Mana {
                cost: cost(3, &[Color::Red]),
                at_hand: 1,
                missing: Vec::new(),
                tax: 2,
            })
        ))
    );
}

#[rstest]
#[case::flashback_in_upkeep(Step::Upkeep, 2, Unplayable::Timing, Some(Why::SorcerySpeed))]
#[case::flashback_short_of_mana(Step::Main1, 1, Unplayable::Mana, Some(Why::Mana { cost: cost(1, &[Color::Blue]), at_hand: 1, missing: Vec::new(), tax: 0 }))]
fn a_graveyard_card_with_flashback_says_why_and_a_spent_spell_says_nothing(
    #[values(2, 3, 4)] players: usize,
    #[case] step: Step,
    #[case] lands: usize,
    #[case] reason: Unplayable,
    #[case] why: Option<Why>,
) {
    let mut driver = Driver::from_scenario(
        seats(players, step)
            .graveyard(0, [card("Deep Analysis"), card("Opt")])
            .battlefield(0, (0..lands).map(|_| ready(island()))),
    )
    .unwrap();
    // A table in its upkeep may first ask about the turn's triggers; wait for seat 0's priority.
    while driver.decision().unwrap().player != PlayerId(0)
        || !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
    {
        driver.answer_passively().unwrap();
    }
    let graveyard = Zone::Graveyard(PlayerId(0));
    let analysis = in_zone(&driver, graveyard, "Deep Analysis");
    let opt = in_zone(&driver, graveyard, "Opt");
    assert_eq!(told(&driver, analysis), Some((reason, why)));
    assert_eq!(told(&driver, opt), None, "a spent spell has no way back");
}

#[rstest]
fn the_top_of_the_library_says_why_only_under_a_permission(
    #[values(2, 3, 4)] players: usize,
    #[values(true, false)] permitted: bool,
) {
    let mut scenario = seats(players, Step::Upkeep);
    if permitted {
        scenario = scenario.battlefield(0, [ready(future_sight())]);
    }
    let driver = Driver::from_scenario(scenario.library(0, [card("Grizzly Bears")])).unwrap();
    let top = *driver
        .state()
        .zone(Zone::Library(PlayerId(0)))
        .last()
        .unwrap();
    assert_eq!(
        driver.state().object(top).unwrap().card().name,
        "Grizzly Bears"
    );
    let expected = permitted.then_some((Unplayable::Timing, Some(Why::SorcerySpeed)));
    assert_eq!(told(&driver, top), expected);
}
