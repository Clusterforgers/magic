//! Goal: a cost reduction for face-down creature spells applies to a creature cast face down and to no other cast of it, so a morph card with no mana is castable face down and not face up, with the log replaying to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay, ways};

fn ape() -> Arc<CardDefinition> {
    let mut def = CardDefinition::new("Masked Ape")
        .with_type(CardType::Creature)
        .with_power_toughness(3, 3);
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{1}{G}").unwrap());
    def.rules = vec![Arc::new(Ability::Keyword(Keyword::Morph(Cost::mana(
        "{G}".parse().unwrap(),
    ))))];
    Arc::new(def)
}

fn warden(applies: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"CostModifier":{{"applies_to":{applies},"change":"Reduce","mana":"{{3}}"}}}}"#
    );
    Arc::new(
        CardDefinition::new("Warden")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

/// Whether the Ape can be cast face down with the Warden's rule and no lands, and whether it can be cast face up with
/// one Forest.
fn casts(applies: &str) -> (bool, bool) {
    let table = |lands: usize| {
        Scenario::new(2)
            .hand(0, vec![ape()])
            .battlefield(0, {
                let mut mine = vec![ready(warden(applies))];
                mine.extend((0..lands).map(|_| ready(forest())));
                mine
            })
            .library(0, vec![forest(); 12])
            .library(1, vec![forest(); 12])
    };
    let mut free = Driver::from_scenario(table(0)).unwrap();
    let down = free.begin_cast("Masked Ape").is_ok()
        && (!matches!(free.decision().unwrap().kind, DecisionKind::ChooseOption(_))
            || choose_way(&mut free, "Cast face down").is_ok())
        && pay(&mut free).is_ok();
    let state = replay(free.engine().setup(), free.engine().events()).unwrap();
    assert_eq!(state.state_hash(), free.state().state_hash());
    let mut one = Driver::from_scenario(table(1)).unwrap();
    let up = one.begin_cast("Masked Ape").is_ok()
        && match one.decision().unwrap().kind {
            DecisionKind::ChooseOption(_) => ways(&one)
                .unwrap()
                .iter()
                .any(|way| way.starts_with("Cast for")),
            _ => false,
        };
    (down, up)
}

#[rstest]
#[case::face_down_filter(r#"["Spell",{"Type":"Creature"},"FaceDown"]"#, true, false)]
#[case::face_down_and_control(
    r#"["Spell",{"Type":"Creature"},"FaceDown",{"ControlledBy":"You"}]"#,
    true,
    false
)]
#[case::without_the_filter_every_cast_is_cheaper(r#"["Spell",{"Type":"Creature"}]"#, true, true)]
#[case::another_type_changes_nothing(r#"["Spell",{"Type":"Artifact"}]"#, false, false)]
fn the_face_down_filter_matches_only_a_face_down_cast(
    #[case] applies: &str,
    #[case] down: bool,
    #[case] up: bool,
) {
    assert_eq!(casts(applies), (down, up));
}
