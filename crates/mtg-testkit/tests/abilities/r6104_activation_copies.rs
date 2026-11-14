//! Goal: an activated ability is copied by what its payment did, at two to four seats: Ashnod the Uncaring may copy an
//! ability of an artifact or creature whose cost sacrificed a permanent (the source or another), not one that sacrificed
//! nothing or of another kind of permanent; Verrak, Warped Sengir lets its controller pay the life the cost paid
//! (its own life or Phyrexian mana paid with life) again to copy the ability, and nothing happens for a cost of mana
//! alone; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// A permanent of `kind` named `name` with "<cost>: It deals 1 damage to target player."
fn r6104_pinger(name: &str, kind: CardType, cost: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_type(kind);
    if kind == CardType::Creature {
        card = card.with_power_toughness(1, 1);
    }
    Arc::new(
        card.with_rules(
            serde_json::from_str(&format!(
                r#"{{"Activated":{{"cost":{cost},"targets":[{{"what":"Player"}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":1}}}}]}}}}"#
            ))
            .unwrap(),
        ),
    )
}

/// Activates `name` at seat 1, sacrificing `fodder` when the cost asks, answers each yes-or-no question and each payment
/// of the trigger with `yes`, and lets everything resolve.
fn r6104_activate(driver: &mut Driver, name: &str, fodder: &str, yes: bool) {
    driver.begin_activation(name).unwrap();
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::ChooseTargets(_) => {
                let at = driver.target_player(1);
                driver.choose_targets(&[at]).unwrap();
            }
            DecisionKind::ChooseCards(_) => {
                let id = driver.id_of(fodder).unwrap();
                driver
                    .answer(Answer::ChooseCards(CardsChoice(vec![id])))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes))).unwrap();
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().resolving().is_none()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return;
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the stack does not empty");
}

fn r6104_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn r6104_replays(scenario: Scenario, driver: &Driver) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
#[case::an_artifact_sacrificing_itself("Bomb", CardType::Artifact, r#"["SacrificeSelf"]"#, true)]
#[case::a_creature_sacrificing_another(
    "Altar",
    CardType::Creature,
    r#"[{"Sacrifice":{"filter":[{"Type":"Creature"},"Other"]}}]"#,
    true
)]
#[case::an_artifact_sacrificing_nothing("Rod", CardType::Artifact, r#""{1}""#, false)]
#[case::an_enchantment_sacrificing_itself(
    "Rite",
    CardType::Enchantment,
    r#"["SacrificeSelf"]"#,
    false
)]
fn r6104_ashnod_copies_an_ability_paid_by_a_sacrifice(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] kind: CardType,
    #[case] cost: &str,
    #[case] copied: bool,
    #[values(true, false)] accept: bool,
) {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Ashnod the Uncaring").unwrap()),
            ready(r6104_pinger(name, kind, cost)),
            ready(creature("Fodder", 0, &[], 1, 1)),
            ready(swamp()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    r6104_activate(&mut driver, name, "Fodder", accept);
    let dealt = if copied && accept { 2 } else { 1 };
    assert_eq!(r6104_life(&driver, 1), 20 - dealt);
    r6104_replays(scenario, &driver);
}

#[rstest]
#[case::its_own_life(r#"[{"PayLife":2}]"#, 0, Some(2))]
#[case::phyrexian_mana_paid_with_life(r#""{B/P}""#, 0, Some(2))]
#[case::phyrexian_mana_paid_with_mana(r#""{B/P}""#, 1, None)]
#[case::mana_alone(r#""{B}""#, 1, None)]
#[case::life_and_a_sacrifice(
    r#"[{"PayLife":1},{"Sacrifice":{"filter":[{"Type":"Creature"},"Other"]}}]"#,
    0,
    Some(1)
)]
fn r6104_verrak_copies_for_the_life_paid_again(
    #[values(2, 3, 4)] seats: usize,
    #[case] cost: &str,
    #[case] swamps: usize,
    #[case] life_paid: Option<i32>,
    #[values(true, false)] accept: bool,
) {
    let mut board = vec![
        ready(card("Verrak, Warped Sengir").unwrap()),
        ready(r6104_pinger("Pinger", CardType::Artifact, cost)),
    ];
    board.extend((0..swamps).map(|_| ready(swamp())));
    board.push(ready(creature("Fodder", 0, &[], 1, 1)));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    r6104_activate(&mut driver, "Pinger", "Fodder", accept);
    let (lost, dealt) = match life_paid {
        Some(paid) if accept => (2 * paid, 2),
        Some(paid) => (paid, 1),
        None => (0, 1),
    };
    assert_eq!(r6104_life(&driver, 0), 20 - lost, "life paid");
    assert_eq!(r6104_life(&driver, 1), 20 - dealt, "damage dealt");
    r6104_replays(scenario, &driver);
}
