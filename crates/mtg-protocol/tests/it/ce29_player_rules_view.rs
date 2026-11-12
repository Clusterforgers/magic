//! Goal: an Aura on a player names in `ObjectView.gives` the rules of the game it gives the enchanted player (can't cast spells, a cast limit, can't activate, another prohibition, a skipped draw step, no draws, no life gain), to every seat alike, and nothing for a rule about other players or one whose condition fails.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::gives::GivenRule;
use mtg_protocol::{GivesView, StateView};
use mtg_testkit::{Driver, Scenario, enchanting, ready};
use rstest::rstest;

/// A curse on seat 1 whose one other ability is the rule ability `rule` (data).
fn curse(rule: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Probe Curse")
        .with_type(CardType::Enchantment)
        .with_subtype("Aura");
    for ability in [r#"{"Keyword":{"EnchantPlayer":{"opponent":false}}}"#, rule] {
        card = card.with_rules(serde_json::from_str::<Ability>(ability).unwrap());
    }
    Arc::new(card)
}

fn rules(rules: &[GivenRule]) -> GivesView {
    GivesView {
        power: 0,
        toughness: 0,
        set: None,
        keywords: Vec::new(),
        rules: rules.to_vec(),
    }
}

#[rstest]
#[case::cant_cast(r#"{"Rule":{"rule":{"CantCastSpells":{"who":"AttachedPlayer"}}}}"#, Some(rules(&[GivenRule::CantCastSpells])))]
#[case::cant_cast_by_prohibition(r#"{"Rule":{"rule":{"Cant":{"who":"AttachedPlayer","action":{"Cast":{}}}}}}"#, Some(rules(&[GivenRule::CantCastSpells])))]
#[case::cast_limit(r#"{"Rule":{"rule":{"Cant":{"who":"AttachedPlayer","action":{"Cast":{"more_than":1}}}}}}"#, Some(rules(&[GivenRule::CastLimit])))]
#[case::cant_activate(r#"{"Rule":{"rule":{"Cant":{"who":"AttachedPlayer","action":{"ActivateKind":{"kind":"AnyButLoyalty"}}}}}}"#, Some(rules(&[GivenRule::CantActivate])))]
#[case::cant_sacrifice(r#"{"Rule":{"rule":{"Cant":{"who":"AttachedPlayer","action":"Sacrifice"}}}}"#, Some(rules(&[GivenRule::PlayerCant])))]
#[case::skips_draws(r#"{"Rule":{"rule":{"SkipDrawStep":"AttachedPlayer"}}}"#, Some(rules(&[GivenRule::SkipsDrawStep])))]
#[case::cant_draw(r#"{"Rule":{"rule":{"CantDraw":"AttachedPlayer"}}}"#, Some(rules(&[GivenRule::CantDraw])))]
#[case::cant_gain_life(r#"{"Rule":{"rule":{"CantGainLife":"AttachedPlayer"}}}"#, Some(rules(&[GivenRule::CantGainLife])))]
#[case::about_the_controller(r#"{"Rule":{"rule":{"CantGainLife":"You"}}}"#, None)]
#[case::a_failed_condition(
    r#"{"Rule":{"rule":{"CantDraw":"AttachedPlayer"},"condition":{"Not":"YourTurn"}}}"#,
    None
)]
fn an_aura_on_a_player_names_the_rules_it_gives(
    #[case] rule: &str,
    #[case] expected: Option<GivesView>,
) {
    let permanents = [enchanting(ready(curse(rule)), 1)];
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(0, permanents)).unwrap();
    for viewer in 0..2 {
        let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
        let aura = view
            .objects
            .iter()
            .find(|o| o.name == "Probe Curse")
            .unwrap();
        assert_eq!(aura.gives, expected, "seat {viewer}");
    }
}
