//! Goal: a permanent that chooses a basic land type or one of a list of card types as it enters offers exactly those, and its static abilities read the type it chose ("lands you control are the chosen type in addition", "each creature of the chosen type has flying", "enchanted land is the chosen type") at 2, 3 and 4 seats.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Subtype};
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::has_keyword;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

/// A land that asks for `choice` as it enters and has the static ability `rule`.
fn chooser(choice: Value, rule: Value) -> Arc<CardDefinition> {
    let rules: Vec<Ability> = [
        json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": {"Choose": choice}}}),
        json!({"Static": rule}),
    ]
    .into_iter()
    .map(|rule| serde_json::from_value(rule).unwrap())
    .collect();
    let mut card = CardDefinition::new("Chooser").with_type(CardType::Land);
    for rule in rules {
        card = card.with_rules(rule);
    }
    Arc::new(card)
}

fn forest() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Grove")
            .with_type(CardType::Land)
            .with_subtype("Forest"),
    )
}

fn bear() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Bear")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2),
    )
}

fn relic() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact))
}

/// Plays the chooser and picks the option labelled `pick`; the labels offered.
fn play_and_pick(driver: &mut Driver, pick: &str) -> Vec<String> {
    driver.play_land("Chooser").unwrap();
    let decision = driver.decision().unwrap().clone();
    let DecisionKind::ChooseOption(options) = &decision.kind else {
        panic!("no choice was asked: {decision:?}");
    };
    let labels: Vec<String> = options
        .options
        .iter()
        .map(|option| match option {
            ChoiceOption::Label(label) => label.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    let at = labels.iter().position(|label| label == pick).unwrap();
    driver
        .answer(Answer::ChooseOption(OptionChoice(at)))
        .unwrap();
    labels
}

fn subtypes(driver: &Driver, name: &str) -> Vec<String> {
    let id = driver.id_of(name).unwrap();
    let printed = &driver.state().object(id).unwrap().card().subtypes;
    let chars = characteristics(driver.state(), id).unwrap();
    ["Plains", "Island", "Swamp", "Mountain", "Forest"]
        .into_iter()
        .filter(|land| chars.has_subtype(printed, &Subtype((*land).to_owned())))
        .map(str::to_owned)
        .collect()
}

#[rstest]
#[case::in_addition(json!(["AddChosenSubtype"]), &["Island", "Forest"])]
#[case::instead(json!([{"SetLandTypes": []}, "AddChosenSubtype"]), &["Island"])]
fn a_basic_land_type_changes_the_lands(
    #[values(2, 3, 4)] seats: usize,
    #[case] mods: Value,
    #[case] after: &[&str],
) {
    let rule = json!({"affected": {"Each": [{"Type": "Land"}, {"ControlledBy": "You"}, {"Not": {"Named": "Chooser"}}]}, "mods": mods});
    let scenario = Scenario::new(seats)
        .hand(0, vec![chooser(json!("BasicLandType"), rule)])
        .battlefield(0, [ready(forest())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let offered = play_and_pick(&mut driver, "Island");
    assert_eq!(offered, ["Plains", "Island", "Swamp", "Mountain", "Forest"]);
    assert_eq!(subtypes(&driver, "Grove"), after);
}

#[rstest]
#[case::creatures_fly("Creature", true, false)]
#[case::artifacts_fly("Artifact", false, true)]
#[case::nothing_of_the_type("Sorcery", false, false)]
fn a_card_type_from_a_list_is_read_by_the_group(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: &str,
    #[case] bear_flies: bool,
    #[case] relic_flies: bool,
) {
    let set: mtg_core::card::CardTypes = [
        CardType::Artifact,
        CardType::Creature,
        CardType::Enchantment,
        CardType::Instant,
        CardType::Sorcery,
    ]
    .into_iter()
    .collect();
    let rule = json!({"affected": {"Each": ["ChosenSubtype"]}, "mods": [{"AddKeyword": "Flying"}]});
    let scenario = Scenario::new(seats)
        .hand(0, vec![chooser(json!({"CardTypeOf": set}), rule)])
        .battlefield(0, [ready(bear()), ready(relic())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let offered = play_and_pick(&mut driver, pick);
    assert_eq!(
        offered,
        ["Artifact", "Creature", "Enchantment", "Instant", "Sorcery"]
    );
    let flies = |name: &str| {
        let id = driver.id_of(name).unwrap();
        has_keyword(driver.state(), id, &Keyword::Flying)
    };
    assert_eq!(flies("Bear"), bear_flies);
    assert_eq!(flies("Relic"), relic_flies);
}
