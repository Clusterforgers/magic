//! Goal: the parameters that restrict a target (the controller the targets share) or shape a zone change read once for every verb; each either means what the card prints or stays a gap named after the key.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::{abilities, gaps};

fn permanent(script: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject").with_type(CardType::Artifact);
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn line(body: &str) -> String {
    format!("Name:T\nTypes:Sorcery\n{body}\n")
}

/// "Whenever a creature dies, return target card from its controller's graveyard to its owner's hand."
const DIES: &str = "Name:Subject\nTypes:Artifact\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Creature | TriggerZones$ Battlefield | Execute$ E\nSVar:E:DB$ ChangeZone | Origin$ Graveyard | Destination$ Hand | ValidTgts$ Card | TargetsWithDefinedController$ TriggeredCardController\n";

/// Kills `Theirs` (seat 1) and returns the driver at the trigger's target choice.
fn killed() -> Driver {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Bolt", 3)])
        .graveyard(0, [free_creature("Mine In Yard", 1, 1)])
        .graveyard(1, [free_creature("Theirs In Yard", 1, 1)])
        .battlefield(0, [ready(permanent(DIES)), ready(mountain())])
        .battlefield(1, [ready(free_creature("Theirs", 1, 1))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Theirs").unwrap();
    driver.begin_cast("Bolt").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    for _ in 0..40 {
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::ChooseTargets(_)
        ) {
            return driver;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the trigger never asked for a target");
}

#[test]
fn the_card_of_the_controller_in_the_graveyard_is_a_legal_target() {
    let mut driver = killed();
    let theirs = find(&driver, 1, "Theirs In Yard");
    assert!(driver.choose_targets(&[theirs]).is_ok());
}

#[test]
fn a_card_of_another_player_is_no_legal_target() {
    let mut driver = killed();
    let mine = find(&driver, 0, "Mine In Yard");
    assert!(driver.choose_targets(&[mine]).is_err());
}

fn find(driver: &Driver, seat: usize, name: &str) -> Target {
    let state = driver.state();
    let object = state
        .zone(mtg_engine::zones::Zone::Graveyard(
            mtg_core::ids::PlayerId::from_index(seat),
        ))
        .iter()
        .copied()
        .filter_map(|id| state.object(id))
        .find(|object| object.card().name == name)
        .unwrap();
    Target::Object(object.reference())
}

#[rstest]
#[case::trigger_word(
    "T:Mode$ DamageDone | ValidSource$ Creature | ValidTarget$ Player | Execute$ E | TriggerZones$ Battlefield\nSVar:E:DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | ValidTgts$ Creature | TargetsWithDefinedController$ TriggeredTarget",
    &[]
)]
#[case::unknown_word(
    "A:SP$ GainLife | ValidTgts$ Player | LifeAmount$ 1 | SubAbility$ B\nSVar:B:DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | ValidTgts$ Creature | TargetsWithDefinedController$ Foo",
    &["key:TargetsWithDefinedController=Foo"]
)]
#[case::parent_target_is_engine_work(
    "A:SP$ GainLife | ValidTgts$ Player | LifeAmount$ 1 | SubAbility$ B\nSVar:B:DB$ ChangeZone | Origin$ Battlefield | Destination$ Hand | ValidTgts$ Creature | TargetsWithDefinedController$ ParentTarget",
    &["key:TargetsWithDefinedController=ParentTarget"]
)]
fn controller_restriction_words(#[case] body: &str, #[case] expected: &[&str]) {
    assert_eq!(gaps(&line(body)), expected);
}

fn sorcery(script: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject")
        .with_type(CardType::Sorcery)
        .with_mana_cost(mtg_testkit::cards::cost(0, &[mtg_core::color::Color::Red]));
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

/// The target loses 4 life unless it pays {X}, X being the creatures the caster controls (two).
const UNLESS: &str = "Name:Subject\nTypes:Sorcery\nManaCost:R\nA:SP$ LoseLife | ValidTgts$ Player | LifeAmount$ 4 | UnlessCost$ X | UnlessPayer$ Targeted\nSVar:X:Count$Valid Creature.YouCtrl\n";

fn life_after(lands: usize, pays: bool) -> i32 {
    use mtg_engine::decision::Answer;
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [sorcery(UNLESS)])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(free_creature("A", 1, 1)),
                ready(free_creature("B", 1, 1)),
            ],
        )
        .battlefield(1, (0..lands).map(|_| ready(mountain())));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_player(1);
    driver.begin_cast("Subject").unwrap();
    driver.choose_targets(&[target]).unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if idle => break,
            DecisionKind::YesNo(_) => Answer::YesNo(mtg_engine::choose::YesNoChoice(pays)),
            DecisionKind::PayMana(_) => Answer::PayMana(mtg_engine::decision::PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    driver
        .state()
        .player(mtg_core::ids::PlayerId::from_index(1))
        .unwrap()
        .life()
}

#[rstest]
#[case::pays_the_amount(2, true, 20)]
#[case::declines(2, false, 16)]
fn an_unless_cost_of_an_amount_is_paid_that_many_times(
    #[case] lands: usize,
    #[case] pays: bool,
    #[case] life: i32,
) {
    assert_eq!(life_after(lands, pays), life);
}

#[rstest]
#[case::life("PayLife<X>")]
#[case::energy("PayEnergy<X>")]
#[case::mana("X")]
fn an_amount_cost_reads_its_svar(#[case] cost: &str) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ GainLife | LifeAmount$ 2 | UnlessCost$ {cost} | UnlessPayer$ You\nSVar:X:Count$Valid Creature.YouCtrl\n"
    );
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
}
