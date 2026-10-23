//! Goal: the bots never declare an attack the engine refuses for its cost (CR 508.1h), at 2 to 4 seats: the budget of the attack options counts a permanent once however many mana abilities it has (a land of six), what a bot declares within it is accepted for a generic tax and for a Phyrexian symbol at a low life total, and an attack the budget allows but the colours of the sources cannot pay is taken back to what the player can pay.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, EngineError, PresetPermanent};
use mtg_testkit::bot::apply_answer;
use mtg_testkit::cards::{free_creature, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{RandomBot, Scenario, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// How many seeds each bot test tries.
const SEEDS: u64 = 60;

/// A land with a mana ability for each of the five colours and colourless, of which a payment taps one.
fn prism_land() -> Arc<CardDefinition> {
    let mut land = CardDefinition::new("Prism Land").with_type(CardType::Land);
    for mana in ["White", "Blue", "Black", "Red", "Green", "Colorless"] {
        let ability = format!(
            r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"Fixed":["{mana}"]}}}}}}]}}}}"#
        );
        land = land.with_rules(serde_json::from_str::<Ability>(&ability).unwrap());
    }
    Arc::new(land)
}

fn prisms(count: usize) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(prism_land())).collect()
}

/// Seat 0 with `life` is about to declare, with `creatures` named A, B and C (the first ones) and `mine` besides;
/// seat 1 holds the corpus cards `held`.
fn table(
    seats: usize,
    held: &[&str],
    mine: Vec<PresetPermanent>,
    creatures: usize,
    life: i32,
) -> Engine {
    let mut battlefield: Vec<PresetPermanent> = ["A", "B", "C"]
        .iter()
        .take(creatures)
        .map(|name| ready(free_creature(name, 1, 1)))
        .collect();
    battlefield.extend(mine);
    let held: Vec<PresetPermanent> = held.iter().map(|name| ready(card(name).unwrap())).collect();
    Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .life(0, life)
        .battlefield(0, battlefield)
        .battlefield(1, held)
        .build()
        .unwrap()
}

/// Every creature of seat 0 offered attacks seat 1.
fn everyone(engine: &Engine) -> Answer {
    let DecisionKind::DeclareAttackers(options) = &engine.decision().unwrap().kind else {
        panic!("the declaration is asked");
    };
    Answer::DeclareAttackers(
        options
            .attackers
            .iter()
            .map(|option| (option.object, AttackTarget::Player(pid(1))))
            .collect(),
    )
}

#[rstest]
#[case::one_prism_land(1, 0, 1)]
#[case::a_prism_land_and_two_plains(1, 2, 3)]
#[case::two_prism_lands_and_a_plains(2, 1, 3)]
#[case::only_plains(0, 4, 4)]
fn the_budget_counts_a_permanent_once_however_many_abilities_it_has(
    #[values(2, 3, 4)] seats: usize,
    #[case] prism: usize,
    #[case] plain: usize,
    #[case] mana: u32,
) {
    let mut mine = prisms(prism);
    mine.extend(lands(plain, plains));
    let engine = table(seats, &["Propaganda"], mine, 2, 20);
    let DecisionKind::DeclareAttackers(options) = &engine.decision().unwrap().kind else {
        panic!("the declaration is asked");
    };
    assert_eq!(options.mana, mana);
}

/// Seeds whose bot answer is accepted by the engine as it stands, and those that start a payment.
fn bot_answers_without_refusal(build: impl Fn() -> Engine) -> usize {
    let mut paying = 0;
    for seed in 0..SEEDS {
        let mut engine = build();
        let decision = engine.decision().unwrap().clone();
        let answer = RandomBot::new(seed, 0).choose(&decision);
        engine
            .apply(answer)
            .unwrap_or_else(|error| panic!("seed {seed}: the engine refused the bot: {error}"));
        if matches!(
            engine.decision().map(|next| &next.kind),
            Some(DecisionKind::PayMana(_))
        ) {
            paying += 1;
        }
    }
    paying
}

#[rstest]
#[case::a_prism_land_and_a_plains(1, 1)]
#[case::two_prism_lands(2, 0)]
#[case::a_prism_land_and_three_plains(1, 3)]
fn a_bot_within_the_budget_of_a_prism_land_is_never_refused(
    #[values(2, 3, 4)] seats: usize,
    #[case] prism: usize,
    #[case] plain: usize,
) {
    let paying = bot_answers_without_refusal(|| {
        let mut mine = prisms(prism);
        mine.extend(lands(plain, plains));
        table(seats, &["Propaganda"], mine, 3, 20)
    });
    assert!(paying > 0, "some seeds attack and pay");
}

#[rstest]
fn a_bot_at_one_life_pays_a_phyrexian_symbol_with_mana_or_stays_home(
    #[values(2, 3, 4)] seats: usize,
) {
    let paying = bot_answers_without_refusal(|| {
        let mut mine = prisms(1);
        mine.extend(lands(3, plains));
        table(seats, &["Propaganda", "Norn's Annex"], mine, 2, 1)
    });
    assert!(paying > 0, "some seeds attack and pay");
}

#[rstest]
fn an_attack_its_player_cannot_pay_in_colours_is_refused_and_then_taken_back_to_what_it_can_pay(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] plains_count: usize,
) {
    let mut mine = lands(plains_count, plains);
    mine.extend(lands(2 - plains_count, island));
    let mut engine = table(seats, &["Norn's Annex"], mine, 2, 1);
    let decision = engine.decision().unwrap().clone();
    let both = everyone(&engine);
    // Two Islands add up to the {W/P}{W/P} of the budget, though none of it is white.
    let DecisionKind::DeclareAttackers(options) = &decision.kind else {
        panic!("the declaration is asked");
    };
    assert_eq!(options.mana, 2);
    let paid = plains_count.min(2);
    if paid == 2 {
        assert!(engine.apply(both.clone()).is_ok(), "white from both");
        return;
    }
    assert!(matches!(
        engine.apply(both.clone()),
        Err(EngineError::Illegal(IllegalAnswer::AttackUnaffordable(_)))
    ));
    apply_answer(&mut engine, &decision, both).unwrap();
    let next = engine.decision().unwrap();
    match (&next.kind, paid) {
        (DecisionKind::PayMana(options), 1) => {
            assert_eq!(options.cost.mana_value(), 1, "one creature attacks");
        }
        (kind, 0) => assert!(
            !matches!(kind, DecisionKind::PayMana(_)),
            "no creature attacks: {kind:?}"
        ),
        (kind, _) => panic!("unexpected {kind:?}"),
    }
}
