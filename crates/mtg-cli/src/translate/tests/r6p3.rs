//! Goal: a Role token attached to a creature translates to a create and an attach; the one-Role-per-controller rule is the engine's state-based action.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::json;

use super::r3p5::token_card;

#[rstest]
#[case::target("Targeted", "Creature.YouCtrl+Other", json!({"Target": 0}))]
fn a_role_token_is_created_and_attached(
    #[case] to: &str,
    #[case] valid: &str,
    #[case] host: serde_json::Value,
) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Token | TokenScript$ role_test | TokenOwner$ You | AttachedTo${to} | ValidTgts$ {valid}\n"
    );
    let card = token_card(&text);
    assert_eq!(card.gaps().count(), 0);
    let found = card.faces.into_iter().next().unwrap().abilities;
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(effects[1]["Attach"]["to"], host);
    assert_eq!(
        effects[1]["Attach"]["what"],
        json!({"Bound": effects[0]["CreateToken"]["as"].clone()})
    );
}

use super::{abilities, gaps};

#[rstest]
#[case::generic("2", "{2}")]
#[case::colored("2 U", "{2}{U}")]
#[case::two_colors("1 G G", "{1}{G}{G}")]
fn strive_costs_more_for_each_target_beyond_the_first(#[case] cost: &str, #[case] mana: &str) {
    let text = format!(
        "Name:T\nTypes:Instant\nK:Strive:{cost}\nA:SP$ Draw | ValidTgts$ Player | SpellDescription$ Draw.\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    assert_eq!(
        found[0],
        json!({"CostModifier": {"applies_to": ["This"], "change": "Increase", "mana": mana,
            "per": {"Sum": [{"TargetCount": 0}, -1]}}})
    );
}

#[rstest]
#[case::no_cost("K:Strive")]
#[case::not_mana("K:Strive:Sac<1/Creature>")]
fn a_strive_the_rule_cannot_read_is_a_gap(#[case] line: &str) {
    let text = format!("Name:T\nTypes:Instant\n{line}\n");
    assert_eq!(gaps(&text).len(), 1);
}

use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ready};

fn creature(script: &str) -> std::sync::Arc<mtg_cards::CardDefinition> {
    let mut card = mtg_cards::CardDefinition::new("Subject")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2);
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value(value).unwrap());
    }
    std::sync::Arc::new(card)
}

fn settle(driver: &mut Driver) {
    for _ in 0..80 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle
            && matches!(
                driver.engine().decision().map(|d| &d.kind),
                Some(DecisionKind::Priority(_))
            )
        {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

#[rstest]
#[case::one("Flying")]
#[case::the_other("First Strike")]
#[case::both("Flying & First Strike")]
fn a_debuff_takes_the_named_keywords_and_no_others(#[case] lost: &str) {
    let script = format!(
        "Name:Subject\nTypes:Creature Elf\nPT:2/2\nK:Flying\nK:First Strike\nA:AB$ Debuff | Cost$ 0 | Defined$ Self | Keywords$ {lost}\n"
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(creature(&script))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Subject").unwrap();
    driver.pay_tapping(&[], 0).unwrap();
    settle(&mut driver);
    let id = driver.id_of("Subject").unwrap();
    let has = |keyword| mtg_engine::keywords::has_keyword(driver.state(), id, &keyword);
    let (loses_flying, loses_strike) = (lost.contains("Flying"), lost.contains("First Strike"));
    assert_eq!(has(Keyword::Flying), !loses_flying);
    assert_eq!(has(Keyword::FirstStrike), !loses_strike);
}

#[test]
fn a_debuff_of_a_keyword_the_rules_cannot_name_is_a_gap() {
    let script = "Name:T\nTypes:Instant\nA:SP$ Debuff | ValidTgts$ Creature | Keywords$ walk\n";
    assert_eq!(gaps(script).len(), 1);
}

fn protections(driver: &Driver, name: &str) -> usize {
    let id = driver.id_of(name).unwrap();
    mtg_engine::keywords::keywords_of(driver.state(), id)
        .into_iter()
        .filter(|keyword| matches!(keyword, Keyword::Protection(_)))
        .count()
}

#[rstest]
#[case::a_chosen_color("Gains$ Choice | Choices$ AnyColor", 1)]
#[case::a_chosen_card_type("Gains$ Choice | Choices$ CardType", 1)]
#[case::named_colors("Gains$ white,red", 2)]
fn protection_reaches_the_creature_and_no_other(#[case] gains: &str, #[case] count: usize) {
    let script = format!(
        "Name:Subject\nTypes:Creature Elf\nPT:2/2\nA:AB$ Protection | Cost$ 0 | Defined$ Self | {gains}\n"
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    let other = mtg_testkit::cards::free_creature("Other", 1, 1);
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(creature(&script)), ready(other)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Subject").unwrap();
    driver.pay_tapping(&[], 0).unwrap();
    settle(&mut driver);
    assert_eq!(protections(&driver, "Subject"), count);
    assert_eq!(protections(&driver, "Other"), 0);
}

#[test]
fn protection_for_a_group_reaches_only_the_creatures_it_names() {
    let script = "Name:Subject\nTypes:Creature Elf\nPT:2/2\nA:AB$ ProtectionAll | Cost$ 0 | ValidCards$ Creature.YouCtrl | Gains$ Choice | Choices$ AnyColor\n";
    assert_eq!(gaps(script), Vec::<String>::new());
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(creature(script)),
                ready(mtg_testkit::cards::free_creature("Mine", 1, 1)),
            ],
        )
        .battlefield(
            1,
            [ready(mtg_testkit::cards::free_creature("Theirs", 1, 1))],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Subject").unwrap();
    driver.pay_tapping(&[], 0).unwrap();
    settle(&mut driver);
    assert_eq!(protections(&driver, "Subject"), 1);
    assert_eq!(protections(&driver, "Mine"), 1);
    assert_eq!(protections(&driver, "Theirs"), 0);
}

#[rstest]
#[case::two_kinds(
    "Gains$ Choice | Choices$ AnyColor,Artifact",
    "key:Protection.Choices=AnyColor,Artifact"
)]
#[case::the_target_chooses(
    "Gains$ Choice | Choices$ AnyColor | Choser$ Targeted",
    "key:Protection.Choser"
)]
#[case::an_earlier_choice("Gains$ ChosenColor", "key:Protection.Gains=ChosenColor")]
fn a_protection_the_rules_cannot_read_is_a_gap(#[case] gains: &str, #[case] gap: &str) {
    let script =
        format!("Name:T\nTypes:Instant\nA:SP$ Protection | ValidTgts$ Creature | {gains}\n");
    assert_eq!(gaps(&script), [gap]);
}

#[rstest]
#[case::itself("Defined$ Self", json!("This"))]
#[case::the_default("Defined$ Self | Activate$ True", json!("This"))]
fn a_prepare_card_becomes_prepared(#[case] extra: &str, #[case] what: serde_json::Value) {
    let script = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nA:AB$ AlterAttribute | Cost$ 1 | Attributes$ Prepared | {extra}\n"
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script)[0]["Activated"]["effects"][0],
        json!({"Prepare": {"what": what}})
    );
}

#[rstest]
#[case::suspected("Attributes$ Suspected", "key:AlterAttribute.Attributes=Suspected")]
#[case::unprepared(
    "Attributes$ Prepared | Activate$ False",
    "key:AlterAttribute.Activate"
)]
fn an_attribute_the_rules_do_not_run_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script =
        format!("Name:T\nTypes:Creature Elf\nPT:1/1\nA:AB$ AlterAttribute | Cost$ 1 | {line}\n");
    assert_eq!(gaps(&script), [gap]);
}
