//! Goal: combat rules a static ability states are read as the rules say: a rule that creatures with a landwalk ability can be blocked as though they did not have it frees the blockers of the one land it names (CR 702.14), or of every landwalk, or lets named blockers ignore it, and leaves another landwalk alone.

use crate::common;

use std::sync::Arc;

use common::combat::to_blockers;
use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::{CardType, Subtype};
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn walks(land: &str) -> Keyword {
    Keyword::Landwalk(Selector(vec![Pred::Subtype(Subtype(land.to_owned()))]))
}

fn creature(name: &str, keywords: &[Keyword]) -> Arc<CardDefinition> {
    Arc::new(
        keywords.iter().fold(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_power_toughness(2, 3),
            |card, keyword| card.with_rules(Ability::Keyword(keyword.clone())),
        ),
    )
}

fn rule_card(rule: &str) -> Arc<CardDefinition> {
    let ability: Ability =
        serde_json::from_str(&format!(r#"{{"Rule":{{"rule":{rule}}}}}"#)).unwrap();
    Arc::new(
        CardDefinition::new("Rulebook")
            .with_type(CardType::Enchantment)
            .with_rules(ability),
    )
}

/// The blockers offered against "A" when it attacks with `keywords` into a defender with a Forest and a creature "B".
fn offered(keywords: &[Keyword], rule: Option<&str>) -> Result<Vec<String>, ScriptError> {
    let mut mine = vec![ready(creature("A", keywords))];
    mine.extend(rule.map(|rule| ready(rule_card(rule))));
    let mut scenario = Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, mine)
        .battlefield(
            1,
            [ready(creature("B", &[])), ready(forest()), ready(island())],
        );
    for seat in 0..2 {
        scenario = scenario.library(seat, (0..10).map(|_| free_creature("Filler", 1, 1)));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    let Ok(decision) = driver.decision() else {
        return Ok(Vec::new());
    };
    let DecisionKind::DeclareBlockers(options) = &decision.kind else {
        return Ok(Vec::new());
    };
    Ok(options
        .blockers
        .iter()
        .map(|blocker| name_of(driver.state(), blocker.object))
        .collect())
}

const FOREST: &str = r#"[{"Subtype":"Forest"}]"#;
const FOREST_WALKERS: &str = r#"{"Each":[{"HasKeyword":{"Landwalk":[{"Subtype":"Forest"}]}}]}"#;

#[rstest]
#[case::the_walk_alone_is_unblockable(vec![walks("Forest")], None, &[])]
#[case::the_rule_for_that_land_frees_the_blockers(
    vec![walks("Forest")],
    Some(format!(r#"{{"Combat":{{"who":{FOREST_WALKERS},"quirk":{{"IgnoreLandwalk":{FOREST}}}}}}}"#)),
    &["B"]
)]
#[case::the_rule_for_another_land_changes_nothing(
    vec![walks("Forest")],
    Some(r#"{"Combat":{"who":{"Each":[{"Type":"Creature"}]},"quirk":{"IgnoreLandwalk":[{"Subtype":"Island"}]}}}"#.to_owned()),
    &[]
)]
#[case::another_walk_of_the_creature_still_applies(
    vec![walks("Forest"), walks("Island")],
    Some(format!(r#"{{"Combat":{{"who":{FOREST_WALKERS},"quirk":{{"IgnoreLandwalk":{FOREST}}}}}}}"#)),
    &[]
)]
#[case::every_landwalk_ignored(
    vec![walks("Forest"), walks("Island")],
    Some(r#"{"Combat":{"who":{"Each":[{"Type":"Creature"}]},"quirk":"IgnoreAllLandwalk"}}"#.to_owned()),
    &["B"]
)]
#[case::a_named_blocker_ignores_it(
    vec![walks("Forest")],
    Some(r#"{"Combat":{"who":{"Each":[{"Named":"B"}]},"quirk":"BlockIgnoringLandwalk"}}"#.to_owned()),
    &["B"]
)]
fn landwalk_can_be_ignored_by_a_rule(
    #[case] keywords: Vec<Keyword>,
    #[case] rule: Option<String>,
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    assert_eq!(offered(&keywords, rule.as_deref())?, expected);
    Ok(())
}

fn skiff(crew: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Skiff")
            .with_type(CardType::Artifact)
            .with_power_toughness(4, 4)
            .with_rules(Ability::Keyword(Keyword::Crew(crew))),
    )
}

const MEMBER: &str = r#"{"Each":[{"Named":"Member"}]}"#;

#[rstest]
#[case::power_alone_is_too_little(3, (1, 4), None, false)]
#[case::toughness_counts_instead(
    3, (1, 4), Some(format!(r#"{{"Combat":{{"who":{MEMBER},"quirk":{{"TapPower":"Toughness"}}}}}}"#)), true
)]
#[case::toughness_instead_is_not_more_than_power(
    3, (3, 1), Some(format!(r#"{{"Combat":{{"who":{MEMBER},"quirk":{{"TapPower":"Toughness"}}}}}}"#)), false
)]
#[case::two_greater_is_enough(
    3, (1, 1), Some(format!(r#"{{"Combat":{{"who":{MEMBER},"quirk":{{"TapPower":{{"Plus":2}}}}}}}}"#)), true
)]
#[case::two_greater_is_not_always_enough(
    4, (1, 1), Some(format!(r#"{{"Combat":{{"who":{MEMBER},"quirk":{{"TapPower":{{"Plus":2}}}}}}}}"#)), false
)]
fn crewing_counts_the_power_a_rule_gives_the_creature(
    #[case] crew: u32,
    #[case] (power, toughness): (i32, i32),
    #[case] rule: Option<String>,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    // CR 702.122a: the creatures tapped have total power N or more; a rule may change what a creature counts for.
    let mut mine = vec![
        ready(skiff(crew)),
        ready(free_creature("Member", power, toughness)),
    ];
    mine.extend(rule.map(|rule| ready(rule_card(&rule))));
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(0, mine))?;
    assert_eq!(!driver.offered_abilities("Skiff")?.is_empty(), offered);
    Ok(())
}
