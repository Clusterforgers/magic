//! Goal: for every reason a card or ability is not offered, at tables of two to four seats, the offer names the detail (`Why`) read off the same check — the target that has no legal choice in the card's words and its zone, the mana asked against the mana at hand, sorcery speed, the card's own condition, the permanent or effect that forbids it, split second, summoning sickness, a tapped source, a spent limit, a cost besides mana — and a refused play carries the same reason and detail.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Activators, Limit, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::PresetPermanent;
use mtg_engine::decision::{
    Answer, DecisionKind, PriorityAction, PriorityOptions, TargetZone, Unplayable, Why,
};
use mtg_engine::effects::Duration;
use mtg_engine::error::{EngineError, IllegalAnswer};
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, draw, forest, gain_life, island, mountain, one_target, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped};
use rstest::rstest;

fn card(name: &str) -> Arc<CardDefinition> {
    mtg_testkit::corpus::card(name).unwrap()
}

fn rule(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A free instant with one target of `what` that draws a card.
fn targeting(name: &str, what: Vec<Pred>) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Object(Selector(what)))],
                effects: vec![draw(1)],
            }),
    )
}

/// "Copy target creature spell you control", as Double Major targets.
fn double_major() -> Arc<CardDefinition> {
    targeting(
        "Double Major",
        vec![
            Pred::Type(CardType::Creature),
            Pred::Spell,
            Pred::ControlledBy(Ref::You),
        ],
    )
}

/// A free instant that draws a card, cast only during combat.
fn combat_trick() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Combat Trick")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rule(r#"{"CastOnly":{"condition":"InCombat"}}"#))
            .with_rules(rule(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#)),
    )
}

/// A 1/1 creature with one activated ability: `items` as its cost, `timing`, `limit` and one target of `what` if given.
fn engine_with(
    name: &str,
    items: Vec<CostItem>,
    timing: Timing,
    limit: Option<Limit>,
    what: Option<Targetable>,
) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(1, &[]))
            .with_power_toughness(1, 1)
            .with_rules(Ability::Activated {
                cost: Cost(items),
                timing,
                condition: None,
                limit,
                by: Activators::Controller,
                from: ZoneKind::Battlefield,
                targets: what.into_iter().map(one_target).collect(),
                effects: vec![gain_life(1)],
            }),
    )
}

fn tapper() -> Arc<CardDefinition> {
    engine_with("Tapper", vec![CostItem::Tap], Timing::Instant, None, None)
}

/// A table of `players` seats where seat 0 is active in `step` of turn 2, holds `hand`, and controls `mine`; seat 1
/// controls `theirs`.
fn table(
    players: usize,
    step: Step,
    hand: Vec<Arc<CardDefinition>>,
    mine: Vec<PresetPermanent>,
    theirs: Vec<PresetPermanent>,
) -> Driver {
    let mut scenario = Scenario::new(players)
        .at(2, 0, step)
        .hand(0, hand)
        .battlefield(0, mine)
        .battlefield(1, theirs);
    for seat in 0..players {
        scenario = scenario.library(seat, (0..10).map(|_| forest()));
    }
    Driver::from_scenario(scenario).unwrap()
}

fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("not a priority decision: {other:?}"),
    }
}

fn in_hand(driver: &Driver, name: &str) -> ObjectId {
    let state = driver.state();
    *state
        .zone(Zone::Hand(PlayerId(0)))
        .iter()
        .find(|id| state.object(**id).unwrap().card().name == name)
        .unwrap()
}

/// The reason and detail the offer gives for the card `name` of seat 0's hand.
fn told(driver: &Driver, name: &str) -> Option<(Unplayable, Option<Why>)> {
    let id = in_hand(driver, name);
    options(driver)
        .unplayable
        .into_iter()
        .find(|entry| entry.card == id)
        .map(|entry| (entry.reason, entry.why))
}

/// The reason and detail the offer gives for the first ability of the permanent `name`.
fn told_ability(driver: &Driver, name: &str) -> Option<(Unplayable, Option<Why>)> {
    let id = driver.id_of(name).unwrap();
    let options = options(driver);
    assert!(
        options.abilities.iter().all(|ability| ability.object != id),
        "{name} is not offered"
    );
    options
        .inactive
        .into_iter()
        .find(|entry| entry.ability.object == id)
        .map(|entry| (entry.reason, entry.why))
}

fn target(needs: &str, zone: TargetZone) -> Why {
    Why::Target {
        needs: needs.to_owned(),
        zone,
    }
}

type Cards = fn() -> Vec<Arc<CardDefinition>>;
type Permanents = fn() -> Vec<PresetPermanent>;

fn no_permanents() -> Vec<PresetPermanent> {
    Vec::new()
}

#[rstest]
#[case::a_copy_spell_with_nothing_on_the_stack(Step::Main1, || vec![double_major()], || vec![ready(card("Grizzly Bears"))], "Double Major", Unplayable::Targets, Some(target("a creature spell you control on the stack", TargetZone::Stack)))]
#[case::a_counterspell_with_an_empty_stack(Step::Main1, || vec![card("Counterspell")], || vec![ready(island()), ready(island())], "Counterspell", Unplayable::Targets, Some(target("a spell on the stack", TargetZone::Stack)))]
#[case::a_kill_spell_with_no_creature(Step::Main1, || vec![card("Doom Blade")], || vec![ready(swamp()), ready(swamp())], "Doom Blade", Unplayable::Targets, Some(target("a nonblack creature on the battlefield", TargetZone::Battlefield)))]
#[case::a_reanimation_with_an_empty_graveyard(Step::Main1, || vec![targeting("Raise", vec![Pred::Type(CardType::Creature), Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)])], no_permanents, "Raise", Unplayable::Targets, Some(target("a creature card in your graveyard", TargetZone::Graveyard)))]
#[case::a_creature_in_upkeep(Step::Upkeep, || vec![card("Grizzly Bears")], || vec![ready(forest()), ready(forest())], "Grizzly Bears", Unplayable::Timing, Some(Why::SorcerySpeed))]
#[case::a_land_in_upkeep(Step::Upkeep, || vec![forest()], no_permanents, "Forest", Unplayable::Timing, Some(Why::SorcerySpeed))]
#[case::one_mana_short(Step::Main1, || vec![card("Grizzly Bears")], || vec![ready(forest())], "Grizzly Bears", Unplayable::Mana, Some(Why::Mana { cost: cost(1, &[Color::Green]), at_hand: 1, missing: Vec::new() }))]
#[case::the_wrong_colour(Step::Main1, || vec![card("Grizzly Bears")], || vec![ready(island()), ready(island())], "Grizzly Bears", Unplayable::Mana, Some(Why::Mana { cost: cost(1, &[Color::Green]), at_hand: 2, missing: vec![ManaType::Green] }))]
#[case::nothing_to_sacrifice(Step::Main1, || vec![card("Altar's Reap")], || vec![ready(swamp()), ready(swamp())], "Altar's Reap", Unplayable::Cost, Some(Why::OtherCost))]
#[case::cast_only_in_combat(Step::Main1, || vec![combat_trick()], no_permanents, "Combat Trick", Unplayable::Restricted, Some(Why::OwnCondition))]
fn a_card_says_why_in_detail(
    #[values(2, 3, 4)] players: usize,
    #[case] step: Step,
    #[case] hand: Cards,
    #[case] mine: Permanents,
    #[case] name: &str,
    #[case] reason: Unplayable,
    #[case] why: Option<Why>,
) {
    let driver = table(players, step, hand(), mine(), Vec::new());
    assert_eq!(told(&driver, name), Some((reason, why)));
}

#[rstest]
fn a_copy_spell_is_offered_once_a_creature_spell_is_on_the_stack(
    #[values(2, 3, 4)] players: usize,
) {
    let mut driver = table(
        players,
        Step::Main1,
        vec![double_major(), card("Grizzly Bears")],
        vec![ready(forest()), ready(forest())],
        Vec::new(),
    );
    driver.cast("Grizzly Bears").unwrap();
    assert_eq!(told(&driver, "Double Major"), None);
}

#[rstest]
fn a_land_after_the_land_play_has_no_detail(#[values(2, 3, 4)] players: usize) {
    let mut driver = table(
        players,
        Step::Main1,
        vec![forest(), island()],
        Vec::new(),
        Vec::new(),
    );
    driver.play_land("Forest").unwrap();
    assert_eq!(
        told(&driver, "Island"),
        Some((Unplayable::LandPlayed, None))
    );
}

#[rstest]
fn a_permanent_that_forbids_casting_is_named(#[values(2, 3, 4)] players: usize) {
    // Tidal Barracuda: "Your opponents can't cast spells during your turn." Seat 1 is active and passes to seat 0.
    let mut scenario = Scenario::new(players)
        .at(2, 1, Step::Upkeep)
        .hand(0, vec![card("Opt")])
        .battlefield(0, vec![ready(island())])
        .battlefield(1, vec![ready(card("Tidal Barracuda"))]);
    for seat in 0..players {
        scenario = scenario.library(seat, (0..10).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    while driver.decision().unwrap().player != PlayerId(0) {
        driver.pass().unwrap();
    }
    let barracuda = driver.id_of("Tidal Barracuda").unwrap();
    assert_eq!(
        told(&driver, "Opt"),
        Some((
            Unplayable::Restricted,
            Some(Why::ForbiddenBy {
                source: Some(barracuda),
                name: Some("Tidal Barracuda".to_owned()),
            })
        ))
    );
}

#[rstest]
fn an_effect_with_no_object_forbids_without_a_name(#[values(2, 3, 4)] players: usize) {
    let mut driver = table(
        players,
        Step::Main1,
        vec![card("Grizzly Bears")],
        vec![ready(forest()), ready(forest())],
        Vec::new(),
    );
    let forbid = Rule::CantCastSpells {
        who: vec![PlayerId(0)],
        turn: None,
    };
    driver.add_rule(forbid, Duration::UntilEndOfTurn).unwrap();
    assert_eq!(
        told(&driver, "Grizzly Bears"),
        Some((
            Unplayable::Restricted,
            Some(Why::ForbiddenBy {
                source: None,
                name: None,
            })
        ))
    );
}

#[rstest]
fn split_second_names_the_spell_for_cards_and_abilities(#[values(2, 3, 4)] players: usize) {
    let mut driver = table(
        players,
        Step::Main1,
        vec![card("Sudden Shock"), card("Opt")],
        vec![
            ready(mountain()),
            ready(mountain()),
            ready(island()),
            ready(tapper()),
        ],
        Vec::new(),
    );
    let seat = driver.target_player(1);
    driver.begin_cast("Sudden Shock").unwrap();
    driver.choose_targets(&[seat]).unwrap();
    driver.pay_tapping(&["Mountain", "Mountain"], 0).unwrap();
    let shock = driver.state().stack()[0];
    let why = Some(Why::SplitSecond {
        spell: shock,
        name: "Sudden Shock".to_owned(),
    });
    assert_eq!(
        told(&driver, "Opt"),
        Some((Unplayable::Restricted, why.clone()))
    );
    assert_eq!(
        told_ability(&driver, "Tapper"),
        Some((Unplayable::Restricted, why))
    );
}

#[rstest]
#[case::summoning_sick(|| vec![sick(tapper())], "Tapper", Step::Main1, Unplayable::Cost, Some(Why::SummoningSick))]
#[case::already_tapped(|| vec![tapped(tapper())], "Tapper", Step::Main1, Unplayable::Cost, Some(Why::Tapped))]
#[case::sorcery_speed(|| vec![ready(engine_with("Slow", vec![CostItem::Tap], Timing::Sorcery, None, None))], "Slow", Step::Upkeep, Unplayable::Timing, Some(Why::SorcerySpeed))]
#[case::no_spell_to_target(|| vec![ready(engine_with("Redirector", vec![CostItem::Tap], Timing::Instant, None, Some(Targetable::Object(Selector(vec![Pred::Spell])))))], "Redirector", Step::Main1, Unplayable::Targets, Some(target("a spell on the stack", TargetZone::Stack)))]
#[case::too_little_mana(|| vec![ready(engine_with("Pricey", vec![CostItem::Mana("{3}{R}".parse().unwrap())], Timing::Instant, None, None)), ready(mountain())], "Pricey", Step::Main1, Unplayable::Mana, Some(Why::Mana { cost: cost(3, &[Color::Red]), at_hand: 1, missing: Vec::new() }))]
fn an_ability_says_why_in_detail(
    #[values(2, 3, 4)] players: usize,
    #[case] mine: Permanents,
    #[case] name: &str,
    #[case] step: Step,
    #[case] reason: Unplayable,
    #[case] why: Option<Why>,
) {
    let driver = table(players, step, Vec::new(), mine(), Vec::new());
    assert_eq!(told_ability(&driver, name), Some((reason, why)));
}

#[rstest]
fn a_spent_limit_says_so(#[values(2, 3, 4)] players: usize) {
    let once = engine_with(
        "Once",
        vec![CostItem::PayLife(Amount::Fixed(1))],
        Timing::Instant,
        Some(Limit::OncePerTurn),
        None,
    );
    let mut driver = table(
        players,
        Step::Main1,
        Vec::new(),
        vec![ready(once)],
        Vec::new(),
    );
    driver.begin_ability("Once", 0).unwrap();
    while driver.decision().unwrap().player != PlayerId(0)
        || !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
    {
        driver.answer_passively().unwrap();
    }
    assert_eq!(
        told_ability(&driver, "Once"),
        Some((Unplayable::Restricted, Some(Why::LimitSpent)))
    );
}

#[rstest]
fn a_refused_play_carries_the_reason_of_the_offer(#[values(2, 3, 4)] players: usize) {
    let mut driver = table(
        players,
        Step::Main1,
        vec![double_major()],
        vec![tapped(tapper())],
        Vec::new(),
    );
    let id = in_hand(&driver, "Double Major");
    let refused = driver.answer(Answer::Priority(PriorityAction::CastSpell(id)));
    let Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Refused(refusal)))) = refused
    else {
        panic!("the cast is refused with its reason: {refused:?}");
    };
    assert_eq!(refusal.object, id);
    assert_eq!(refusal.reason, Unplayable::Targets);
    assert_eq!(
        refusal.why,
        Some(target(
            "a creature spell you control on the stack",
            TargetZone::Stack
        ))
    );
}
