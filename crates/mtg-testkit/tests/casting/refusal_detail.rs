//! Goal: at tables of two to four seats the refusals that told only their reason name their detail: an Aura or mutating spell with no host names the host it needs, an ability of a card in a graveyard, exile or the command zone says why it cannot be activated now (and a refused activation carries it), an X ability whose every payable X leaves no target names that target and the X it can pay, and a cast taken back for its mana names the total against the mana at hand.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Activators, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::refs::ZoneKind;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::choose::Target;
use mtg_engine::decision::{
    Answer, DecisionKind, HostBy, PriorityAction, PriorityOptions, TargetZone, Unplayable, Why,
};
use mtg_engine::error::{EngineError, IllegalAnswer};
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::{cost, forest, gain_life, island, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

pub(crate) fn card(name: &str) -> Arc<CardDefinition> {
    mtg_testkit::corpus::card(name).unwrap()
}

/// `players` seats with seat 0 active in `step` of turn 2, holding `hand` and controlling `mine`; seat 1 has `theirs` in
/// its graveyard.
pub(crate) fn table(
    players: usize,
    step: Step,
    hand: Vec<Arc<CardDefinition>>,
    mine: Vec<PresetPermanent>,
    theirs: Vec<Arc<CardDefinition>>,
) -> Scenario {
    let mut scenario = Scenario::new(players)
        .at(2, 0, step)
        .hand(0, hand)
        .battlefield(0, mine)
        .graveyard(1, theirs);
    for seat in 0..players {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

pub(crate) fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("not a priority decision: {other:?}"),
    }
}

/// The first card named `name` in `zone`.
pub(crate) fn in_zone(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    let state = driver.state();
    *state
        .zone(zone)
        .iter()
        .find(|id| state.object(**id).unwrap().card().name == name)
        .unwrap()
}

/// The reason and detail the offer gives for the card `name` of seat 0's hand.
pub(crate) fn told_card(driver: &Driver, name: &str) -> Option<(Unplayable, Option<Why>)> {
    let id = in_zone(driver, Zone::Hand(PlayerId(0)), name);
    options(driver)
        .unplayable
        .into_iter()
        .find(|entry| entry.card == id)
        .map(|entry| (entry.reason, entry.why))
}

/// The reason and detail the offer gives for the first ability of `id`, which it does not offer.
pub(crate) fn told_ability(driver: &Driver, id: ObjectId) -> Option<(Unplayable, Option<Why>)> {
    let options = options(driver);
    assert!(
        options.abilities.iter().all(|ability| ability.object != id),
        "not offered"
    );
    options
        .inactive
        .into_iter()
        .find(|entry| entry.ability.object == id)
        .map(|entry| (entry.reason, entry.why))
}

fn host(needs: &str, by: HostBy) -> Why {
    Why::Host {
        needs: needs.to_owned(),
        by,
    }
}

pub(crate) fn lands(count: usize, land: fn() -> Arc<CardDefinition>) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(land())).collect()
}

#[rstest]
#[case::an_aura_with_no_creature(
    "Pacifism",
    lands(2, plains),
    host("a creature on the battlefield", HostBy::Enchant)
)]
#[case::a_green_aura_with_no_creature(
    "Rancor",
    lands(1, forest),
    host("a creature on the battlefield", HostBy::Enchant)
)]
#[case::a_mutate_creature_with_mana_for_mutate_only(
    "Dreamtail Heron",
    lands(4, island),
    host("a non-Human creature you own on the battlefield", HostBy::Mutate)
)]
#[case::a_red_mutate_creature(
    "Porcuparrot",
    lands(3, mountain),
    host("a non-Human creature you own on the battlefield", HostBy::Mutate)
)]
fn a_host_spell_names_the_host_it_lacks(
    #[values(2, 3, 4)] players: usize,
    #[case] name: &str,
    #[case] mine: Vec<PresetPermanent>,
    #[case] why: Why,
) {
    let driver = Driver::from_scenario(table(
        players,
        Step::Main1,
        vec![card(name)],
        mine,
        Vec::new(),
    ))
    .unwrap();
    assert_eq!(
        told_card(&driver, name),
        Some((Unplayable::Targets, Some(why)))
    );
}

/// A card with one ability that works from `from`: `{2}: You gain 1 life.`, at sorcery speed when `sorcery`.
fn worker(name: &str, from: ZoneKind, sorcery: bool) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[]))
            .with_power_toughness(1, 1)
            .with_rules(Ability::Activated {
                cost: Cost(vec![CostItem::Mana("{2}".parse().unwrap())]),
                timing: if sorcery {
                    Timing::Sorcery
                } else {
                    Timing::Instant
                },
                condition: None,
                limit: None,
                by: Activators::Controller,
                from,
                targets: Vec::new(),
                effects: vec![gain_life(1)],
            }),
    )
}

/// Seat 0's own card with an ability that works from `zone`, put there: a graveyard and exile by the table, the
/// command zone as an emblem.
fn placed(players: usize, step: Step, zone: ZoneKind, lands: usize) -> (Driver, ObjectId) {
    let mut scenario = table(
        players,
        step,
        Vec::new(),
        (0..lands).map(|_| ready(mountain())).collect(),
        Vec::new(),
    );
    scenario = match zone {
        ZoneKind::Command => scenario.emblems(
            0,
            [Arc::new(emblem_definition(
                "Relic",
                &worker("Relic", ZoneKind::Command, true)
                    .rules
                    .iter()
                    .map(|rule| (**rule).clone())
                    .collect::<Vec<_>>(),
            ))],
        ),
        _ => scenario.graveyard(0, [worker("Relic", zone, true)]),
    };
    let mut driver = Driver::from_scenario(scenario).unwrap();
    while driver.decision().unwrap().player != PlayerId(0)
        || !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
    {
        driver.answer_passively().unwrap();
    }
    let me = PlayerId(0);
    let id = match zone {
        ZoneKind::Command => in_zone(&driver, Zone::Command(me), "Relic"),
        ZoneKind::Exile => {
            let id = in_zone(&driver, Zone::Graveyard(me), "Relic");
            driver
                .manual(ManualAction::Move {
                    object: id,
                    to: ManualZone::Exile,
                })
                .unwrap();
            in_zone(&driver, Zone::Exile, "Relic")
        }
        _ => in_zone(&driver, Zone::Graveyard(me), "Relic"),
    };
    (driver, id)
}

#[rstest]
fn an_ability_of_another_zone_says_why_and_a_refusal_carries_it(
    #[values(2, 3, 4)] players: usize,
    #[values(ZoneKind::Graveyard, ZoneKind::Exile, ZoneKind::Command)] zone: ZoneKind,
    #[values(
        (Step::Upkeep, 2, Unplayable::Timing, Some(Why::SorcerySpeed)),
        (Step::Main1, 1, Unplayable::Mana, Some(Why::Mana { cost: cost(2, &[]), at_hand: 1, missing: Vec::new(), tax: 0 })),
        (Step::Main1, 2, Unplayable::Timing, None)
    )]
    case: (Step, usize, Unplayable, Option<Why>),
) {
    let (step, lands, reason, why) = case;
    let (mut driver, id) = placed(players, step, zone, lands);
    if why.is_none() {
        // Two Mountains pay for it in a main phase: it is offered, so nothing says why.
        assert!(options(&driver).abilities.iter().any(|a| a.object == id));
        assert!(
            options(&driver)
                .inactive
                .iter()
                .all(|a| a.ability.object != id)
        );
        return;
    }
    assert_eq!(told_ability(&driver, id), Some((reason, why.clone())));
    let ability = options(&driver)
        .inactive
        .into_iter()
        .find(|entry| entry.ability.object == id)
        .unwrap()
        .ability;
    let refused = driver
        .answer(Answer::Priority(PriorityAction::Activate(ability)))
        .unwrap_err();
    let ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Refused(refusal))) = refused else {
        panic!("not refused with a reason: {refused:?}");
    };
    assert_eq!((refusal.reason, refusal.why), (reason, why));
}

/// A graveyard card, the step, the lands at hand and their kind, and the reason and detail its ability gives.
type KeywordCase = (
    &'static str,
    Step,
    usize,
    fn() -> Arc<CardDefinition>,
    Unplayable,
    Option<Why>,
);

#[rstest]
fn graveyard_keywords_say_why(
    #[values(2, 3, 4)] players: usize,
    #[values(
        ("Sacred Cat", Step::Upkeep, 1, plains as fn() -> Arc<CardDefinition>, Unplayable::Timing, Some(Why::SorcerySpeed)),
        ("Dregscape Zombie", Step::Main1, 0, swamp as fn() -> Arc<CardDefinition>, Unplayable::Mana, Some(Why::Mana { cost: cost(0, &[Color::Black]), at_hand: 0, missing: vec![ManaType::Black], tax: 0 })),
        ("Deadbridge Goliath", Step::Main1, 2, forest as fn() -> Arc<CardDefinition>, Unplayable::Mana, Some(Why::Mana { cost: cost(4, &[Color::Green, Color::Green]), at_hand: 2, missing: Vec::new(), tax: 0 }))
    )]
    case: KeywordCase,
) {
    let (name, step, count, land, reason, why) = case;
    let scenario =
        table(players, step, Vec::new(), lands(count, land), Vec::new()).graveyard(0, [card(name)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    while driver.decision().unwrap().player != PlayerId(0)
        || !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
    {
        driver.answer_passively().unwrap();
    }
    let id = in_zone(&driver, Zone::Graveyard(PlayerId(0)), name);
    assert_eq!(told_ability(&driver, id), Some((reason, why)));
}

pub(crate) fn x_target(needs: &str, zone: TargetZone, least: u32, most: u32) -> Why {
    Why::XTarget {
        needs: needs.to_owned(),
        zone,
        least,
        most,
    }
}

#[rstest]
#[case::geth_with_two_swamps("Geth, Lord of the Vault", vec![ready(swamp()), ready(swamp())], vec![card("Serra Angel")], x_target("an artifact or creature card with mana value X in an opponent's graveyard", TargetZone::Graveyard, 0, 1))]
#[case::geth_with_three_swamps("Geth, Lord of the Vault", vec![ready(swamp()), ready(swamp()), ready(swamp())], vec![card("Serra Angel")], x_target("an artifact or creature card with mana value X in an opponent's graveyard", TargetZone::Graveyard, 0, 2))]
#[case::tamiyo_with_three_loyalty("Tamiyo, Compleated Sage", vec![], vec![], x_target("a nonland permanent card with mana value X in your graveyard", TargetZone::Graveyard, 0, 3))]
fn an_x_ability_names_the_target_no_payable_x_has(
    #[values(2, 3, 4)] players: usize,
    #[case] name: &str,
    #[case] mut mine: Vec<PresetPermanent>,
    #[case] theirs: Vec<Arc<CardDefinition>>,
    #[case] why: Why,
) {
    let source = PresetPermanent {
        counters: vec![(CounterKind::Loyalty, 3)],
        ..ready(card(name))
    };
    mine.push(source);
    let mut scenario = table(players, Step::Main1, Vec::new(), mine, theirs);
    if name.starts_with("Tamiyo") {
        scenario = scenario.graveyard(0, [card("Serra Angel")]);
    }
    let driver = Driver::from_scenario(scenario).unwrap();
    let id = driver.id_of(name).unwrap();
    let said = options(&driver)
        .inactive
        .into_iter()
        .filter(|entry| entry.ability.object == id)
        .find(|entry| matches!(entry.why, Some(Why::XTarget { .. })))
        .map(|entry| (entry.reason, entry.why));
    assert_eq!(said, Some((Unplayable::Targets, Some(why))));
}

#[rstest]
#[case::consign_to_dust("Consign to Dust", 5, forest as fn() -> Arc<CardDefinition>, "Sol Ring", cost(4, &[Color::Green, Color::Green]))]
#[case::ajanis_presence("Ajani's Presence", 3, plains as fn() -> Arc<CardDefinition>, "Grizzly Bears", cost(2, &[Color::White, Color::White]))]
fn a_cast_taken_back_for_its_mana_names_the_total(
    #[values(2, 3, 4)] players: usize,
    #[case] name: &str,
    #[case] count: usize,
    #[case] land: fn() -> Arc<CardDefinition>,
    #[case] targets: &str,
    #[case] total: mtg_core::mana::ManaCost,
) {
    // The targets are seat 1's, so they make no mana for seat 0.
    let scenario = table(
        players,
        Step::Main1,
        vec![card(name)],
        lands(count, land),
        Vec::new(),
    )
    .battlefield(1, [ready(card(targets)), ready(card(targets))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before = driver.engine().events().len();
    driver.begin_cast(name).unwrap();
    let state = driver.state();
    let two: Vec<Target> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == targets)
        .map(|object| Target::Object(object.reference()))
        .collect();
    driver.choose_targets(&two).unwrap();
    let logged = &driver.engine().events()[before..];
    let expected = Why::Mana {
        cost: total,
        at_hand: u32::try_from(count).unwrap(),
        missing: Vec::new(),
        tax: 0,
    };
    assert!(
        matches!(logged, [Event::AnnouncementRewound { reason: Unplayable::Mana, why: Some(why), activated: false, .. }] if *why == expected),
        "{logged:?}"
    );
}
