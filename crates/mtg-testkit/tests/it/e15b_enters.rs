//! Goal: a permanent that chooses a basic land type, one of a list of card types or the kind of a counter as it enters offers exactly those; its static abilities read the type it chose ("lands you control are the chosen type in addition", "each creature of the chosen type has flying", "enchanted land is the chosen type") and it enters with one counter of the kind chosen, at 2, 3 and 4 seats.

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
use mtg_testkit::cards::cost;
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

/// A creature that enters with a deathtouch or a lifelink counter of its controller's choice.
fn nipper() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_value(json!({"Replacement": {
        "when": {"Enters": {"what": ["This"]}},
        "by": {"WithCounterOf": [{"Other": "deathtouch"}, {"Other": "lifelink"}]}}}))
    .unwrap();
    Arc::new(
        CardDefinition::new("Nipper")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(1, 1)
            .with_rules(rule),
    )
}

#[rstest]
#[case::deathtouch(0, Keyword::Deathtouch, Keyword::Lifelink)]
#[case::lifelink(1, Keyword::Lifelink, Keyword::Deathtouch)]
fn a_counter_of_the_kind_chosen_as_it_enters(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: usize,
    #[case] has: Keyword,
    #[case] lacks: Keyword,
) {
    let scenario = Scenario::new(seats).hand(0, vec![nipper()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Nipper").unwrap();
    let mut asked = Vec::new();
    for _ in 0..8 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                asked = options.options.clone();
                driver
                    .answer(Answer::ChooseOption(OptionChoice(pick)))
                    .unwrap();
                continue;
            }
            _ if driver.id_of("Nipper").is_ok() => break,
            _ => {
                driver.pass().unwrap();
            }
        }
    }
    assert_eq!(
        asked,
        [
            ChoiceOption::Label("deathtouch counter".to_owned()),
            ChoiceOption::Label("lifelink counter".to_owned())
        ]
    );
    let id = driver.id_of("Nipper").unwrap();
    let counters: u32 = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .map(|(_, n)| n)
        .sum();
    assert_eq!(counters, 1);
    assert!(has_keyword(driver.state(), id, &has));
    assert!(!has_keyword(driver.state(), id, &lacks));
}
