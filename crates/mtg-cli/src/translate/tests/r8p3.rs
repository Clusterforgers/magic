//! Goal: the reflexive trigger of an `ImmediateTrigger` reads the objects it was told to remember, bound when it is set up: it acts on them when the payment is made and does nothing when it is not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::{abilities, gaps};

const ANOINTER: &str = "Name:Subject\nTypes:Creature Angel\nPT:2/2\nT:Mode$ Attacks | ValidCard$ Creature | TriggerZones$ Battlefield | Execute$ Imm\nSVar:Imm:AB$ ImmediateTrigger | Cost$ 3 | Execute$ Counter | RememberObjects$ TriggeredAttackerLKICopy\nSVar:Counter:DB$ PutCounter | CounterType$ P1P1 | CounterNum$ 1 | Defined$ DelayTriggerRememberedLKI\n";

fn card(script: &str) -> Arc<CardDefinition> {
    assert_eq!(gaps(script), Vec::<String>::new());
    let mut card = CardDefinition::new("Subject")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn power(driver: &Driver, name: &str) -> i32 {
    let state = driver.state();
    let id = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .unwrap();
    mtg_engine::characteristics::characteristics(state, id)
        .unwrap()
        .power
        .unwrap()
}

/// Attacks with the subject, answers the payment as `pay` says and lets the stack resolve.
fn attack_and_settle(script: &str, pay: bool) -> i32 {
    let scenario = Scenario::new(2).at(1, 0, Step::Main1).battlefield(
        0,
        std::iter::once(ready(card(script))).chain((0..3).map(|_| ready(forest()))),
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Subject", 1)]).unwrap();
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::YesNo(_) => Answer::YesNo(mtg_engine::choose::YesNoChoice(pay)),
            DecisionKind::PayMana(_) if pay => {
                driver
                    .pay_tapping(&["Forest", "Forest", "Forest"], 0)
                    .unwrap();
                continue;
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    power(&driver, "Subject")
}

#[rstest]
#[case::paid(true, 3)]
#[case::not_paid(false, 2)]
fn the_attacker_the_trigger_remembers_gets_the_counter_only_when_paid(
    #[case] pay: bool,
    #[case] expected: i32,
) {
    assert_eq!(attack_and_settle(ANOINTER, pay), expected);
}

const SACRIFICER: &str = "Name:Subject\nTypes:Creature Angel\nPT:2/2\nT:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ Sac\nSVar:Sac:DB$ Sacrifice | SacValid$ Creature.Other | RememberSacrificed$ True | SubAbility$ Imm\nSVar:Imm:DB$ ImmediateTrigger | ConditionDefined$ RememberedLKI | ConditionPresent$ Creature | RememberObjects$ RememberedLKI | Execute$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ X\nSVar:X:TriggerRemembered$CardPower\n";

/// The reflexive trigger reads the sacrificed creature's power as it was (CR 608.2h), and nothing happens when none is sacrificed.
#[rstest]
#[case::sacrificed(true, 23)]
#[case::nothing_to_sacrifice(false, 20)]
fn the_power_of_the_sacrificed_creature_is_read_by_the_trigger_it_causes(
    #[case] other: bool,
    #[case] life: i32,
) {
    let big = Arc::new(
        CardDefinition::new("Big")
            .with_type(CardType::Creature)
            .with_power_toughness(3, 3),
    );
    let scenario = Scenario::new(2).at(1, 0, Step::Main2).battlefield(
        0,
        std::iter::once(ready(card(SACRIFICER))).chain(other.then(|| ready(big))),
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::End).unwrap();
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::YesNo(_) => Answer::YesNo(mtg_engine::choose::YesNoChoice(true)),
            DecisionKind::ChooseCards(o) => Answer::ChooseCards(mtg_engine::choose::CardsChoice(
                o.from.iter().copied().take(o.max as usize).collect(),
            )),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    let life_now = driver
        .state()
        .player(PlayerId::from_index(0))
        .unwrap()
        .life();
    assert_eq!(life_now, life);
}

fn immediate(remember: &str) -> String {
    format!(
        "Name:T\nTypes:Instant\nA:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.YouOwn | SubAbility$ Imm\nSVar:Imm:DB$ ImmediateTrigger | RememberObjects$ {remember} | Execute$ Hit\nSVar:Hit:DB$ Destroy | Defined$ DelayTriggerRememberedLKI\n"
    )
}

/// A target an earlier step moved is another object (CR 400.7), and a word the translator does not read is unknown.
#[rstest]
#[case::moved_target("Targeted")]
#[case::unknown_word("Discarded")]
fn an_unreadable_remembered_word_is_a_named_gap(#[case] word: &str) {
    assert_eq!(
        gaps(&immediate(word)),
        [format!("key:ImmediateTrigger.RememberObjects={word}")]
    );
}

const EACH_IMPRINTED: &str = "Name:Spell\nTypes:Sorcery\nA:SP$ RepeatEach | UseImprinted$ True | RepeatCards$ Creature.YouCtrl | RepeatSubAbility$ Pump\nSVar:Pump:DB$ Pump | Defined$ Imprinted | NumAtt$ +2\n";

/// A loop that names its pass `Imprinted` acts on each creature in turn.
#[test]
fn a_loop_over_imprinted_acts_on_each_object_of_the_pass() {
    assert_eq!(gaps(EACH_IMPRINTED), Vec::<String>::new());
    let mut spell = CardDefinition::new("Spell")
        .with_type(CardType::Sorcery)
        .with_mana_cost(mtg_testkit::cards::cost(0, &[mtg_core::color::Color::Red]));
    for value in abilities(EACH_IMPRINTED) {
        spell = spell.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [Arc::new(spell)])
        .battlefield(
            0,
            [
                ready(mtg_testkit::cards::bear()),
                ready(mtg_testkit::cards::bear()),
                ready(mtg_testkit::cards::mountain()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast_tapping("Spell", &["Mountain"]).unwrap();
    for _ in 0..20 {
        driver.answer_passively().unwrap();
        if driver.state().stack().is_empty() {
            break;
        }
    }
    let state = driver.state();
    let powers: Vec<i32> = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|id| {
            mtg_engine::characteristics::characteristics(state, *id)
                .unwrap()
                .power
        })
        .collect();
    assert_eq!(powers.iter().filter(|power| **power == 4).count(), 2);
}
