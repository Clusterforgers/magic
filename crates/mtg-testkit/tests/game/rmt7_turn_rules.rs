//! Goal: two rules rmt-7 adds hold at two to four seats and replay to the live state. A static ability that holds during extra turns only (`Condition::ExtraTurn`, Medomai the Ageless: "~ can't attack during extra turns.") keeps its creature from attacking in an extra turn and lets it attack in an ordinary one. A prohibition of the enchanted player (`ActivationKind::AnyButLoyalty`, Overwhelming Splendor: "Enchanted player can't activate abilities that aren't mana abilities or loyalty abilities.") takes away that player's other activated abilities and leaves their loyalty abilities, and another player's.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::{Event, NothingToDo};
use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn card(name: &str, kind: CardType, rules: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_type(kind);
    if kind == CardType::Creature {
        card = card.with_power_toughness(3, 3);
    }
    if kind == CardType::Sorcery {
        card = card.with_mana_cost(mtg_core::mana::ManaCost::parse("{0}").unwrap());
    }
    for rule in rules {
        let ability: Ability = serde_json::from_str(rule).unwrap();
        card = card.with_rules(ability);
    }
    Arc::new(card)
}

const AGELESS: &str =
    r#"{"Static":{"affected":"This","mods":["CantAttack"],"condition":"ExtraTurn"}}"#;
const TIME_WALK: &str = r#"{"Spell":{"effects":[{"ExtraTurn":{}}]}}"#;

#[rstest]
fn rmt7_a_creature_cant_attack_during_extra_turns(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] extra: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(card("Ageless", CardType::Creature, &[AGELESS]))])
        .hand(0, [card("Walk", CardType::Sorcery, &[TIME_WALK])]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    if extra {
        driver.cast("Walk").unwrap();
        while !driver.state().stack().is_empty() {
            driver.pass().unwrap();
        }
        for _ in 0..400 {
            if driver.state().turn().number() == 4 {
                break;
            }
            let decision = driver.decision().unwrap().clone();
            driver.answer(Driver::passive_answer(&decision)).unwrap();
        }
        assert!(driver.state().turn().is_extra());
        // The extra turn goes by without an attack to declare: the one creature can't attack.
        let mut asked = false;
        let begun = driver.engine().events().len();
        while driver.state().turn().number() == 4 {
            let decision = driver.decision().unwrap().clone();
            asked |= matches!(decision.kind, DecisionKind::DeclareAttackers(_));
            driver.answer(Driver::passive_answer(&decision)).unwrap();
        }
        assert!(!asked, "no attack is offered in an extra turn");
        assert!(
            driver.engine().events()[begun..]
                .iter()
                .any(|event| matches!(
                    event,
                    Event::NothingToDo {
                        kind: NothingToDo::Attack,
                        ..
                    }
                ))
        );
    } else {
        driver.pass_until(Step::DeclareAttackers).unwrap();
        driver.attack(&[("Ageless", 1)]).unwrap();
    }
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

const SPLENDOR: &str = r#"{"Rule":{"rule":{"Cant":{"who":"AttachedPlayer","action":{"ActivateKind":{"kind":"AnyButLoyalty"}}}}}}"#;
const PING: &str = r#"{"Activated":{"cost":"{0}","effects":[{"GainLife":{"amount":1}}]}}"#;
const PLUS: &str = r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":1}}]}}"#;

#[rstest]
fn rmt7_the_enchanted_player_keeps_only_mana_and_loyalty_abilities(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] enchanted: usize,
) {
    let mut aura = ready(Arc::new(
        Arc::try_unwrap(card(
            "Splendor",
            CardType::Enchantment,
            &[r#"{"Keyword":{"EnchantPlayer":{}}}"#, SPLENDOR],
        ))
        .unwrap()
        .with_subtype("Aura"),
    ));
    aura.attached_player = Some(enchanted);
    let mut walker = PresetPermanent::new(
        Arc::try_unwrap(card("Walker", CardType::Planeswalker, &[PLUS]))
            .unwrap()
            .with_loyalty(3)
            .into(),
    );
    walker.counters = vec![(CounterKind::Loyalty, 3)];
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [ready(card("Pinger", CardType::Creature, &[PING])), walker],
        )
        .battlefield(1, [aura]);
    let driver = Driver::from_scenario(scenario.clone()).unwrap();
    let pinger = driver.offered_abilities("Pinger").unwrap();
    let loyalty = driver.offered_abilities("Walker").unwrap();
    assert_eq!(loyalty, vec![0], "a loyalty ability stays");
    if enchanted == 0 {
        assert!(pinger.is_empty(), "the enchanted player can't activate it");
    } else {
        assert_eq!(pinger, vec![0], "another player is not enchanted");
    }
}
