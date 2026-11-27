//! Goal: every permanent a state-based action puts into the graveyard carries the rule that did it, and a stored bare
//! `"StateBased"` cause still reads, as an unnamed rule.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use rstest::rstest;

use crate::choose::CardsChoice;
use crate::decision::DecisionKind;
use crate::events::{DeathCause, Event, StateRule, ZoneCause};
use crate::fixtures::{creature, setup};
use crate::setup::PresetPermanent;
use crate::state::GameState;

const ENCHANT_CREATURE: &str = r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}}"#;

fn aura(name: &str, role: bool) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Enchantment)
        .with_rules(serde_json::from_str::<Ability>(ENCHANT_CREATURE).unwrap())
        .with_subtype("Aura");
    if role {
        card = card.with_subtype("Role");
    }
    Arc::new(card)
}

fn legend(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_supertype(Supertype::Legendary),
    )
}

/// Seat 0's permanents for each case; a permanent attached gives the place of its host.
fn permanents(case: &str) -> Vec<PresetPermanent> {
    let plain = |card| vec![PresetPermanent::new(card)];
    match case {
        "zero toughness" => plain(creature("Shade", 1, 0)),
        "lethal damage" => {
            let mut bear = PresetPermanent::new(creature("Bear", 2, 2));
            bear.damage = 2;
            vec![bear]
        }
        "zero loyalty" => plain(Arc::new(
            CardDefinition::new("Resonance")
                .with_type(CardType::Planeswalker)
                .with_loyalty(0),
        )),
        "legend rule" => vec![
            PresetPermanent::new(legend("Relic")),
            PresetPermanent::new(legend("Relic")),
        ],
        "unattached aura" => plain(aura("Halo", false)),
        "zero defense" => {
            let mut battle = PresetPermanent::new(Arc::new(
                CardDefinition::new("Siege")
                    .with_type(CardType::Battle)
                    .with_defense(0),
            ));
            battle.protector = Some(1);
            vec![battle]
        }
        "extra role" => {
            let mut older = PresetPermanent::new(aura("Royal", true));
            older.attached_to = Some((0, 0));
            let mut newer = PresetPermanent::new(aura("Wicked", true));
            newer.attached_to = Some((0, 0));
            vec![PresetPermanent::new(creature("Bear", 2, 2)), older, newer]
        }
        other => panic!("no case {other}"),
    }
}

/// The rule of each graveyard move of one pass; the legend rule's question is answered by keeping the first.
fn rules_applied(game: &GameState) -> Vec<StateRule> {
    let mut events = crate::sba::check(game);
    if events.is_empty()
        && let Some(decision) = crate::legend::decision(game)
        && let DecisionKind::ChooseCards(options) = &decision.kind
    {
        let keep = CardsChoice(vec![options.from[0]]);
        let Ok(crate::answers::Outcome::Events(answered)) =
            crate::legend::answer(game, options, &keep)
        else {
            panic!("the legend rule answers with events");
        };
        events = answered;
    }
    events
        .iter()
        .filter_map(|event| match event {
            Event::CreatureDied { cause, .. } => Some(StateRule::Died(*cause)),
            Event::ZoneChanged {
                cause: ZoneCause::StateBased(rule),
                ..
            } => Some(*rule),
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::zero_toughness("zero toughness", StateRule::Died(DeathCause::ZeroToughness))]
#[case::lethal_damage("lethal damage", StateRule::Died(DeathCause::LethalDamage))]
#[case::zero_loyalty("zero loyalty", StateRule::ZeroLoyalty)]
#[case::legend_rule("legend rule", StateRule::LegendRule)]
#[case::unattached_aura("unattached aura", StateRule::UnattachedAura)]
#[case::zero_defense("zero defense", StateRule::ZeroDefense)]
#[case::extra_role("extra role", StateRule::ExtraRole)]
fn a_state_based_action_names_its_rule(#[case] case: &str, #[case] rule: StateRule) {
    let mut setup = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
    setup.players[0].battlefield = permanents(case);
    let game = GameState::from_setup(&setup).unwrap();
    assert_eq!(rules_applied(&game), [rule]);
}

#[rstest]
#[case::bare(r#""StateBased""#, ZoneCause::StateBased(StateRule::Unnamed))]
#[case::named(
    r#"{"StateBased":"ZeroLoyalty"}"#,
    ZoneCause::StateBased(StateRule::ZeroLoyalty)
)]
#[case::died(
    r#"{"StateBased":{"Died":"Deathtouch"}}"#,
    ZoneCause::StateBased(StateRule::Died(DeathCause::Deathtouch))
)]
#[case::other(r#""Sacrificed""#, ZoneCause::Sacrificed)]
fn a_stored_zone_change_reads_its_cause(#[case] cause: &str, #[case] expected: ZoneCause) {
    let json = format!(
        r#"{{"ZoneChanged":{{"object":1,"from":"Battlefield","to":{{"Graveyard":0}},"cause":{cause}}}}}"#
    );
    let Event::ZoneChanged { cause, .. } = serde_json::from_str::<Event>(&json).unwrap() else {
        panic!("a zone change");
    };
    assert_eq!(cause, expected);
}
