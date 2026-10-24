//! Goal: a static ability that names the cards of a hand, a graveyard or exile ("Each creature card in your hand has cycling {1}{U}") gives them its keywords, types and abilities through the same pass as permanents: only the cards its selector names, only its controller's when it says "your", only while its source is on the battlefield, and no permanent is changed by a selector that names another zone.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::tests::rules;
use crate::characteristics::characteristics;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, setup};
use crate::fold::apply_event;
use crate::keywords::keywords_of;
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// A creature card that gives `mods` to the cards `selector` names, in `from`'s zone.
fn lord(selector: &str, mods: &str) -> Arc<CardDefinition> {
    let ability = format!(r#"{{"Static":{{"affected":{{"Each":[{selector}]}},"mods":[{mods}]}}}}"#);
    Arc::new(
        CardDefinition::new("Lord")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rules(&ability)),
    )
}

const CYCLING: &str = r#"{"AddKeyword":{"Cycling":"{1}{U}"}}"#;

/// A game where seat 0 has the lord in play, a creature card in each of the zones and a land card in hand, and seat 1
/// has a creature card in hand and in a graveyard.
fn table(lord: Arc<CardDefinition>) -> GameState {
    let mut setup = setup(
        vec![
            vec![creature("Mine in hand", 2, 2), land_card()],
            vec![creature("Theirs in hand", 2, 2)],
        ],
        vec![vec![lord, creature("Mine in play", 2, 2)], vec![]],
    );
    setup.players[0].graveyard = vec![creature("Mine in graveyard", 2, 2)];
    setup.players[1].graveyard = vec![creature("Theirs in graveyard", 2, 2)];
    GameState::from_setup(&setup).unwrap()
}

fn land_card() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new("Plain land").with_type(CardType::Land))
}

fn named(game: &GameState, name: &str) -> ObjectId {
    game.objects()
        .find(|object| object.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn cycles(game: &GameState, name: &str) -> bool {
    keywords_of(game, named(game, name))
        .iter()
        .any(|keyword| matches!(keyword, Keyword::Cycling(_)))
}

#[rstest]
#[case::the_hand_of_its_controller(
    r#"{"Type":"Creature"},{"In":"Hand"},{"OwnedBy":"You"}"#,
    &["Mine in hand"],
)]
#[case::the_graveyard_of_its_controller(
    r#"{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}"#,
    &["Mine in graveyard"],
)]
#[case::the_hands_of_every_player(
    r#"{"Type":"Creature"},{"In":"Hand"}"#,
    &["Mine in hand", "Theirs in hand"],
)]
#[case::the_cards_of_a_type_only(
    r#"{"Type":"Land"},{"In":"Hand"},{"OwnedBy":"You"}"#,
    &["Plain land"],
)]
#[case::no_permanent_when_the_zone_is_another(
    r#"{"Type":"Creature"},{"In":"Exile"}"#,
    &[],
)]
fn a_static_ability_gives_a_keyword_to_the_cards_of_a_zone(
    #[case] selector: &str,
    #[case] given: &[&str],
) {
    let game = table(lord(selector, CYCLING));
    for name in [
        "Mine in hand",
        "Theirs in hand",
        "Mine in graveyard",
        "Theirs in graveyard",
        "Mine in play",
        "Plain land",
        "Lord",
    ] {
        assert_eq!(cycles(&game, name), given.contains(&name), "{name}");
    }
}

#[test]
fn a_card_gets_the_keyword_in_exile_too_and_loses_it_when_the_source_leaves() {
    let mut game = table(lord(
        r#"{"Type":"Creature"},{"In":"Exile"},{"OwnedBy":"You"}"#,
        CYCLING,
    ));
    let card = named(&game, "Mine in hand");
    let moved = Event::ZoneChanged {
        object: card,
        from: Zone::Hand(ME),
        to: Zone::Exile,
        cause: ZoneCause::Exiled,
    };
    apply_event(&mut game, &moved).unwrap();
    let card = game
        .zone(Zone::Exile)
        .iter()
        .copied()
        .find(|id| {
            game.object(*id)
                .is_some_and(|o| o.card().name == "Mine in hand")
        })
        .unwrap();
    assert!(
        keywords_of(&game, card)
            .iter()
            .any(|k| matches!(k, Keyword::Cycling(_)))
    );
    // The lord leaves: the card is as printed again.
    let lord = named(&game, "Lord");
    let left = Event::ZoneChanged {
        object: lord,
        from: Zone::Battlefield,
        to: Zone::Graveyard(ME),
        cause: ZoneCause::Destroyed,
    };
    apply_event(&mut game, &left).unwrap();
    assert_eq!(keywords_of(&game, card), Vec::<&Keyword>::new());
}

#[rstest]
#[case::a_type(r#"{"AddType":"Artifact"}"#, CardType::Artifact, "Mine in hand")]
#[case::a_type_in_a_graveyard(r#"{"AddType":"Artifact"}"#, CardType::Artifact, "Mine in graveyard")]
fn a_static_ability_changes_the_characteristics_of_the_cards_it_names(
    #[case] change: &str,
    #[case] kind: CardType,
    #[case] name: &str,
) {
    let game = table(lord(
        r#"{"Type":"Creature"},{"OwnedBy":"You"},{"Not":{"In":"Battlefield"}}"#,
        change,
    ));
    // A selector with no zone looks at the battlefield: the cards outside it are as printed.
    assert!(
        !characteristics(&game, named(&game, name))
            .unwrap()
            .types
            .contains(kind)
    );
    let game = table(lord(
        r#"{"Type":"Creature"},{"In":"Hand"},{"OwnedBy":"You"}"#,
        change,
    ));
    let in_hand = characteristics(&game, named(&game, "Mine in hand")).unwrap();
    assert!(in_hand.types.contains(kind));
    let foe = characteristics(&game, named(&game, "Theirs in hand")).unwrap();
    assert!(!foe.types.contains(kind));
}
