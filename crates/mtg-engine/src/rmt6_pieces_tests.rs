//! Goal: a creature blocks the flyers a rule names as though it had reach and no other flyer; a permanent that isn't
//! legendary while it is a token escapes the legend rule as a token and not as a card; at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::{CardType, Supertype};
use mtg_core::ids::ObjectId;
use rstest::rstest;

use crate::fixtures::state;
use crate::object::ObjectKind;
use crate::state::GameState;
use crate::zones::Zone;

fn flyer(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_power_toughness(4, 4)
            .with_rules(Ability::Keyword(Keyword::Flying)),
    )
}

fn hunter(rule: bool) -> Arc<CardDefinition> {
    let card = CardDefinition::new("Hunter")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 3);
    Arc::new(if rule {
        card.with_rules(
            serde_json::from_str(
                r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":{"BlockAsThoughReach":[{"Subtype":"Dragon"}]}}}}}"#,
            )
            .unwrap(),
        )
    } else {
        card
    })
}

fn find(game: &GameState, name: &str) -> ObjectId {
    game.zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            game.object(*id)
                .is_some_and(|object| object.card().name == name)
        })
        .unwrap()
}

#[rstest]
fn the_rule_answers_flying_of_the_named_attackers_only(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        ("Dragon", true, false),
        ("Bird", true, true),
        ("Dragon", false, true)
    )]
    case: (&str, bool, bool),
) {
    let (subtype, rule, stopped) = case;
    let mut boards = vec![Vec::new(); seats];
    boards[0].push(flyer("Flyer", subtype));
    boards[seats - 1].push(hunter(rule));
    let game = state(vec![Vec::new(); seats], boards);
    let (attacker, blocker) = (find(&game, "Flyer"), find(&game, "Hunter"));
    assert_eq!(
        crate::keywords::stops_block(&game, attacker, blocker),
        stopped
    );
}

fn ooze() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Ooze")
            .with_type(CardType::Creature)
            .with_supertype(Supertype::Legendary)
            .with_power_toughness(0, 0)
            .with_rules(
                serde_json::from_str(
                    r#"{"Static":{"affected":"This","mods":[{"RemoveSupertype":"Legendary"}],"condition":{"Matches":{"subject":"This","selector":["Token"]}}}}"#,
                )
                .unwrap(),
            ),
    )
}

#[rstest]
fn a_token_is_not_legendary_and_a_card_is(
    #[values(2, 4)] seats: usize,
    #[values((0, 1), (1, 0), (2, 0))] case: (usize, usize),
) {
    let (tokens, groups) = case;
    let mut boards = vec![Vec::new(); seats];
    boards[0] = vec![ooze(), ooze()];
    let mut game = state(vec![Vec::new(); seats], boards);
    let ids: Vec<ObjectId> = game.zone(Zone::Battlefield).to_vec();
    for id in ids.iter().take(tokens) {
        game.objects.get_mut(id).unwrap().kind = ObjectKind::Token;
    }
    assert_eq!(crate::legend::groups(&game).len(), groups);
    let legendary =
        |id: &ObjectId| crate::characteristics::has_supertype(&game, *id, Supertype::Legendary);
    assert_eq!(
        ids.iter().filter(|id| legendary(id)).count(),
        ids.len() - tokens
    );
}
