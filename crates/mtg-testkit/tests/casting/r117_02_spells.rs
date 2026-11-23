//! Goal: the r117-02 spell pieces play at two to four seats: "if this spell is the first spell you've cast this game" holds only for the caster's first spell, also when they cast another in response (`Condition::R117FirstSpellThisGame`, CR 601.2i); "end the combat phase" exiles the stack, removes attackers from combat and goes on at the postcombat main phase without the end of combat step, and does nothing outside combat (`Effect::R117EndCombat`); a ward cost with a choice ("Ward—Discard a card or pay {2}") lets the targeting player take either cost or none (CR 702.21a); "Artifact spells you cast have replicate {2}{U}" (Hugh, First of Many) offers the replicate payments for an artifact spell only and each copy of the permanent spell enters as a token (CR 702.56, 707.10f); each game replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{ChoiceOption, NumberChoice, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, replay};
use mtg_testkit::cards::{cost, forest, free_creature, island};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::spell;

fn assert_replays(engine: &Engine, driver: &Driver) {
    let state = replay(engine.setup(), engine.events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Answers every decision with `yes` for a may, until the stack is empty and a player has priority.
fn settle(driver: &mut Driver, yes: bool) {
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return;
        }
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(yes)),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the stack never emptied");
}

fn first_life() -> Arc<CardDefinition> {
    spell(
        "Greeting",
        CardType::Instant,
        "[]",
        r#"[{"If":{"condition":"R117FirstSpellThisGame","then":[{"GainLife":{"amount":2}}]}}]"#,
    )
}

fn trick() -> Arc<CardDefinition> {
    spell(
        "Trick",
        CardType::Instant,
        "[]",
        r#"[{"GainLife":{"amount":1}}]"#,
    )
}

/// Which spells are cast, in order, and whether the second is cast in response to the first.
#[derive(Debug, Clone, Copy)]
enum Order {
    GreetingAlone,
    TrickResolvedFirst,
    TrickInResponse,
}

#[rstest]
#[case::alone_two_seats(2, Order::GreetingAlone, 22)]
#[case::alone_four_seats(4, Order::GreetingAlone, 22)]
#[case::after_another_spell(2, Order::TrickResolvedFirst, 21)]
#[case::another_in_response_keeps_it_first(3, Order::TrickInResponse, 23)]
fn the_first_spell_of_the_game_is_the_casters_first(
    #[case] seats: usize,
    #[case] order: Order,
    #[case] life: i32,
) {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, vec![first_life(), trick()]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    match order {
        Order::GreetingAlone => {
            driver.cast("Greeting").unwrap();
            settle(&mut driver, true);
        }
        Order::TrickResolvedFirst => {
            driver.cast("Trick").unwrap();
            settle(&mut driver, true);
            driver.cast("Greeting").unwrap();
            settle(&mut driver, true);
        }
        Order::TrickInResponse => {
            driver.cast("Greeting").unwrap();
            driver.cast("Trick").unwrap();
            settle(&mut driver, true);
        }
    }
    let state = driver.state();
    assert_eq!(state.player(PlayerId(0)).unwrap().life(), life);
    assert_replays(driver.engine(), &driver);
}

fn mandate() -> Arc<CardDefinition> {
    spell(
        "Mandate",
        CardType::Instant,
        "[]",
        r#"[{"Untap":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},{"If":{"condition":"InCombat","then":[{"May":{"effects":["R117EndCombat"]}}]}}]"#,
    )
}

#[rstest]
#[case::ends_combat_two_seats(2, Step::DeclareAttackers, true)]
#[case::ends_combat_after_blockers_four_seats(4, Step::DeclareBlockers, true)]
#[case::declined(2, Step::DeclareAttackers, false)]
#[case::outside_combat(3, Step::Main1, true)]
fn ending_the_combat_phase_exiles_the_stack_and_goes_to_the_postcombat_main_phase(
    #[case] seats: usize,
    #[case] step: Step,
    #[case] yes: bool,
) {
    let start = if step == Step::Main1 {
        Step::Main1
    } else {
        Step::DeclareAttackers
    };
    let mut scenario = Scenario::new(seats)
        .at(2, 0, start)
        .hand(0, vec![trick(), mandate()])
        .battlefield(0, vec![ready(free_creature("Bear", 2, 2))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    if step != Step::Main1 {
        driver.attack(&[("Bear", 1)]).unwrap();
    }
    if step == Step::DeclareBlockers {
        driver.pass_until(Step::DeclareBlockers).unwrap();
    }
    driver.cast("Trick").unwrap();
    driver.cast("Mandate").unwrap();
    settle(&mut driver, yes);
    let ended = yes && step != Step::Main1;
    let state = driver.state();
    let exiled = state.zone(Zone::Exile).len();
    assert_eq!(exiled, if ended { 2 } else { 0 }, "both spells are exiled");
    assert_eq!(
        state.player(PlayerId(0)).unwrap().life(),
        if ended { 20 } else { 21 }
    );
    if ended {
        assert_eq!(
            state.turn().step(),
            Step::Main2,
            "the end of combat step is skipped"
        );
        assert_eq!(state.combat().attackers().len(), 0);
        let bear = driver.id_of("Bear").unwrap();
        assert!(
            !state.object(bear).unwrap().permanent().unwrap().tapped(),
            "the creatures were untapped first"
        );
    }
    if step != Step::Main1 {
        driver.pass_until(Step::End).unwrap();
        assert_eq!(
            driver.state().player(PlayerId(1)).unwrap().life(),
            if ended { 20 } else { 18 },
            "an attack removed from combat deals no damage"
        );
    }
    assert_replays(driver.engine(), &driver);
}

fn warded() -> Arc<CardDefinition> {
    let ward: Ability =
        serde_json::from_str(r#"{"Keyword":{"Ward":[{"OneOf":[[{"Discard":{}}],"{2}"]}]}}"#)
            .unwrap();
    Arc::new(
        CardDefinition::new("Warded")
            .with_type(CardType::Creature)
            .with_power_toughness(3, 3)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(ward),
    )
}

/// What the targeting player pays for ward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pays {
    Discard,
    Mana,
    Nothing,
}

#[rstest]
#[case::discards(2, Pays::Discard)]
#[case::pays_mana(2, Pays::Mana)]
#[case::pays_nothing(2, Pays::Nothing)]
#[case::four_seats_discards(4, Pays::Discard)]
#[case::four_seats_pays_mana(4, Pays::Mana)]
fn a_ward_with_a_choice_of_costs_takes_either_or_counters(
    #[case] seats: usize,
    #[case] pays: Pays,
) {
    let zap = spell(
        "Zap",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"DealDamage":{"to":{"Target":0},"amount":1}}]"#,
    );
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, vec![zap, trick()])
        .battlefield(0, vec![ready(forest()), ready(forest())])
        .battlefield(1, vec![ready(warded())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Warded").unwrap();
    driver.cast_at("Zap", &[target]).unwrap();
    let mut offered = Vec::new();
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseOption(options) if decision.player == PlayerId(0) => {
                offered = options.options.clone();
                let wanted = match pays {
                    Pays::Discard => "iscard",
                    Pays::Mana => "{2}",
                    Pays::Nothing => "none",
                };
                let at = offered
                    .iter()
                    .position(|o| matches!(o, ChoiceOption::Label(l) if l.contains(wanted)))
                    .unwrap_or_else(|| panic!("{offered:?}"));
                Answer::ChooseOption(OptionChoice(at))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    assert_eq!(offered.len(), 3, "{offered:?}");
    let state = driver.state();
    let warded = driver.id_of("Warded").unwrap();
    let damaged = state.object(warded).unwrap().permanent().unwrap().damage() > 0;
    assert_eq!(damaged, pays != Pays::Nothing);
    assert_eq!(
        state.zone(Zone::Hand(PlayerId(0))).len(),
        usize::from(pays != Pays::Discard)
    );
    assert_replays(driver.engine(), &driver);
}

fn gizmo(kind: CardType) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Gizmo")
            .with_type(kind)
            .with_mana_cost(cost(1, &[])),
    )
}

#[rstest]
#[case::artifact_twice(2, CardType::Artifact, Some(2))]
#[case::artifact_once(3, CardType::Artifact, Some(1))]
#[case::artifact_not_paid(4, CardType::Artifact, Some(0))]
#[case::an_enchantment_gets_none(2, CardType::Enchantment, None)]
fn hugh_gives_artifact_spells_replicate(
    #[case] seats: usize,
    #[case] kind: CardType,
    #[case] times: Option<u32>,
) {
    let mut battlefield = vec![ready(corpus::card("Hugh, First of Many").unwrap())];
    battlefield.extend((0..8).map(|_| ready(island())));
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, vec![gizmo(kind)])
        .battlefield(0, battlefield);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Gizmo").unwrap();
    let mut asked = false;
    for _ in 0..80 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::ChooseNumber(options) => {
                asked = true;
                // Eight Islands pay {1} and two more {2}{U}.
                assert_eq!(options.max, Some(2));
                Answer::ChooseNumber(NumberChoice::Number(times.unwrap()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
                continue;
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    assert_eq!(
        asked,
        times.is_some(),
        "replicate is asked of artifact spells only"
    );
    let state = driver.state();
    let gizmos: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == "Gizmo")
        .collect();
    assert_eq!(gizmos.len(), 1 + times.unwrap_or(0) as usize);
    let tokens = gizmos
        .iter()
        .filter(|object| object.kind() == mtg_engine::object::ObjectKind::Token)
        .count();
    assert_eq!(
        tokens,
        times.unwrap_or(0) as usize,
        "each copy becomes a token"
    );
    assert_replays(driver.engine(), &driver);
}

#[rstest]
#[case::a_card_to_discard(2, true, 2)]
#[case::an_empty_hand_draws_nothing(3, false, 0)]
fn discard_a_card_if_you_do_draw_two(
    #[case] seats: usize,
    #[case] held: bool,
    #[case] hand: usize,
) {
    let study = spell(
        "Study",
        CardType::Sorcery,
        "[]",
        r#"[{"Discard":{}},{"If":{"condition":"Performed","then":[{"Draw":{"count":2}}]}}]"#,
    );
    let mut cards = vec![study];
    if held {
        cards.push(trick());
    }
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1).hand(0, cards);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Study").unwrap();
    settle(&mut driver, true);
    assert_eq!(driver.state().zone(Zone::Hand(PlayerId(0))).len(), hand);
    assert_replays(driver.engine(), &driver);
}
