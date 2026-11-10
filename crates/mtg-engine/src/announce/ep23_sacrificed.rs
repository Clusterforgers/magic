//! Goal: a spell's own reduction "for each other artifact or creature you've sacrificed this turn" (Dargo, the
//! Shipwrecker) counts only the permanents of that kind its caster sacrificed before the total cost, beside the reduction
//! for each permanent sacrificed for the cost (X): another player's sacrifices and lands do not count, and the generic
//! mana never goes below zero.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::TallyEvent;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use rstest::rstest;

use super::modifiers::modified;
use crate::choose::Choices;
use crate::fixtures::state;
use crate::state::GameState;

const BY_X: &str = r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{1}","per":{"Times":{"by":2,"of":"X"}}}}"#;
const BY_TALLY: &str = r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{1}","per":{"Times":{"by":2,"of":{"Tally":{"event":"Sacrificed","filter":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}],"who":"You"}}}}}}"#;

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

fn rule(text: &str) -> Ability {
    serde_json::from_str(text).unwrap()
}

fn permanent(name: &str, kind: CardType) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(kind))
}

/// Seat 0 holds Dargo ({6}{R}) beside two creatures, an artifact and a land; seat 1 has a creature of their own.
fn game() -> GameState {
    let dargo = CardDefinition::new("Dargo")
        .with_type(CardType::Creature)
        .with_power_toughness(7, 5)
        .with_rules(rule(BY_X))
        .with_rules(rule(BY_TALLY));
    state(
        vec![vec![Arc::new(dargo)], vec![]],
        vec![
            vec![
                permanent("Bear", CardType::Creature),
                permanent("Ogre", CardType::Creature),
                permanent("Relic", CardType::Artifact),
                permanent("Field", CardType::Land),
            ],
            vec![permanent("Theirs", CardType::Creature)],
        ],
    )
}

fn named(game: &GameState, name: &str) -> mtg_core::ids::ObjectId {
    game.objects()
        .find(|object| object.card().name == name)
        .unwrap()
        .id()
}

#[rstest]
#[case::nothing_sacrificed(&[], None, "{6}{R}")]
#[case::one_creature_before(&[(ME, "Bear")], None, "{4}{R}")]
#[case::a_creature_and_an_artifact_before(&[(ME, "Bear"), (ME, "Relic")], None, "{2}{R}")]
#[case::a_land_does_not_count(&[(ME, "Field")], None, "{6}{R}")]
#[case::another_player_s_does_not_count(&[(FOE, "Theirs")], None, "{6}{R}")]
#[case::one_before_and_one_for_the_cost(&[(ME, "Bear")], Some(1), "{2}{R}")]
#[case::never_below_zero(&[(ME, "Bear"), (ME, "Ogre"), (ME, "Relic")], Some(2), "{R}")]
fn ep23_the_spell_counts_what_its_caster_sacrificed_this_turn(
    #[case] before: &[(PlayerId, &str)],
    #[case] x: Option<u32>,
    #[case] expected: &str,
) {
    let mut game = game();
    for (seat, name) in before {
        let id = named(&game, name);
        game.log_event(TallyEvent::Sacrificed, *seat, id);
    }
    let spell = named(&game, "Dargo");
    let choices = Choices {
        x,
        ..Choices::default()
    };
    let found = modified(
        &game,
        PlayerId(0),
        spell,
        &[],
        &choices,
        Some(ManaCost::parse("{6}{R}").unwrap()),
        None,
    );
    assert_eq!(found, Some(ManaCost::parse(expected).unwrap()));
}
