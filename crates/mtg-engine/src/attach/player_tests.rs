//! Goal: an Aura that enchants a player is legal on a player in the game, on an opponent of its controller when it enchants opponents only, and never when it is a creature or no Aura; becoming attached to a player replaces a host and takes a new timestamp, a player out of the game is refused and names no Aura, becoming unattached clears the player, an Aura card targets a player or an opponent through its enchant ability, and the state-based actions put an Aura of a player who left, or of an opponent-only Aura on its controller, into its owner's graveyard and keep the others.
use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_core::card::Subtype;
use rstest::rstest;

use super::*;
use crate::fixtures::{creature, setup};
use crate::fold::apply_event;
use crate::player::LossReason;
use crate::setup::PresetPermanent;

const ME: PlayerId = PlayerId(0);
const THEM: PlayerId = PlayerId(1);
const THIRD: PlayerId = PlayerId(2);

fn aura(name: &str, rules: &str, creature_too: bool) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_type(CardType::Enchantment);
    if creature_too {
        card = card.with_type(CardType::Creature);
    }
    card.subtypes.push(Subtype("Aura".into()));
    Arc::new(card.with_rules(serde_json::from_str::<Ability>(rules).unwrap()))
}

const FOR_PLAYERS: &str = r#"{"Keyword":{"EnchantPlayer":{}}}"#;
const FOR_OPPONENTS: &str = r#"{"Keyword":{"EnchantPlayer":{"opponent":true}}}"#;

/// Seat 0, in this order: a Curse that enchants players, a Hex that enchants opponents, a Bestowed that enchants players
/// and is a creature, and a Bear. Seats 1 and 2 have nothing. Each pair says which seat the permanent of that place starts
/// enchanting.
fn game(enchanting: &[(usize, usize)]) -> GameState {
    let mut game = setup(
        vec![vec![], vec![], vec![]],
        vec![
            vec![
                aura("Curse", FOR_PLAYERS, false),
                aura("Hex", FOR_OPPONENTS, false),
                aura("Bestowed", FOR_PLAYERS, true),
                creature("Bear", 2, 2),
            ],
            vec![],
            vec![],
        ],
    );
    // Every Aura starts on seat 1 unless the case says otherwise.
    for place in 0..3 {
        let seat = enchanting
            .iter()
            .find(|(at, _)| *at == place)
            .map_or(1, |(_, seat)| *seat);
        let preset = &mut game.players[0].battlefield[place];
        *preset = PresetPermanent {
            attached_player: Some(seat),
            ..preset.clone()
        };
    }
    GameState::from_setup(&game).unwrap()
}

fn id(game: &GameState, name: &str) -> ObjectId {
    game.objects().find(|o| o.card().name == name).unwrap().id()
}

fn lose(game: &mut GameState, player: PlayerId) {
    apply_event(
        game,
        &Event::PlayerLost {
            player,
            reason: LossReason::Conceded,
        },
    )
    .unwrap();
}

#[rstest]
#[case::players_on_yourself("Curse", ME, true)]
#[case::players_on_an_opponent("Curse", THEM, true)]
#[case::opponents_on_an_opponent("Hex", THEM, true)]
#[case::opponents_on_yourself("Hex", ME, false)]
#[case::a_creature_that_is_an_aura("Bestowed", THEM, false)]
#[case::a_creature_that_is_no_aura("Bear", THEM, false)]
fn an_aura_is_legal_on_the_players_its_enchant_ability_allows(
    #[case] attachment: &str,
    #[case] player: PlayerId,
    #[case] legal: bool,
) {
    let game = game(&[]);
    assert_eq!(
        legal_player_host(&game, id(&game, attachment), player),
        legal
    );
}

#[test]
fn a_player_who_left_the_game_is_no_legal_host() {
    let mut game = game(&[]);
    let curse = id(&game, "Curse");
    lose(&mut game, THIRD);
    assert!(legal_player_host(&game, curse, THEM));
    assert!(!legal_player_host(&game, curse, THIRD));
}

#[test]
fn becoming_attached_to_a_player_replaces_the_host_and_takes_the_next_timestamp() {
    let mut game = game(&[]);
    let (curse, bear) = (id(&game, "Curse"), id(&game, "Bear"));
    let before = game.next_timestamp;
    let stamp = |game: &GameState| game.object(curse).unwrap().permanent().unwrap().timestamp();
    let to_bear = Event::Attached {
        object: curse,
        to: bear,
    };
    let to_player = |player| Event::AttachedToPlayer {
        object: curse,
        player,
    };
    apply_event(&mut game, &to_bear).unwrap();
    apply_event(&mut game, &to_player(THEM)).unwrap();
    assert_eq!(
        (game.host_of(curse), game.player_host_of(curse)),
        (None, Some(THEM))
    );
    assert_eq!(
        (stamp(&game), game.next_timestamp),
        (before + 1, before + 2)
    );
    assert!(game.attachments_of_player(THEM).contains(&curse));
    assert_eq!(game.attachments_of(bear), []);
    // Another host takes the player off it.
    apply_event(&mut game, &to_bear).unwrap();
    assert_eq!(
        (game.host_of(curse), game.player_host_of(curse)),
        (Some(bear), None)
    );
    apply_event(&mut game, &to_player(ME)).unwrap();
    apply_event(&mut game, &Event::Unattached { object: curse }).unwrap();
    assert_eq!(game.player_host_of(curse), None);
    assert!(apply_event(&mut game, &Event::Unattached { object: curse }).is_err());
}

#[test]
fn an_aura_cannot_become_attached_to_the_same_player_twice_or_to_one_who_left() {
    let mut game = game(&[(0, 1)]);
    let curse = id(&game, "Curse");
    let attach = |player| Event::AttachedToPlayer {
        object: curse,
        player,
    };
    assert!(apply_event(&mut game, &attach(THEM)).is_err());
    lose(&mut game, THIRD);
    assert!(apply_event(&mut game, &attach(THIRD)).is_err());
    assert_eq!(game.player_host_of(curse), Some(THEM));
}

#[rstest]
#[case::players("Curse", Targetable::Player)]
#[case::opponents("Hex", Targetable::Opponent)]
fn an_aura_card_targets_a_player_or_an_opponent(#[case] aura: &str, #[case] expected: Targetable) {
    let game = game(&[]);
    let spec = spell_target(&game, id(&game, aura)).unwrap();
    assert_eq!(spec.what, expected);
    assert!(spec.bounds.is_one());
}

#[test]
fn an_aura_of_players_put_onto_the_battlefield_without_being_cast_has_no_host_to_choose() {
    let game = game(&[]);
    let curse = game.object(id(&game, "Curse")).unwrap();
    assert_eq!(hosts_when_entering(&game, curse, ME), Some(Vec::new()));
}

#[rstest]
#[case::a_player_in_the_game(&[(0, 1)], &[])]
#[case::the_player_who_left(&[(0, 2)], &["Curse"])]
#[case::an_opponent_only_aura_on_its_controller(&[(1, 0)], &["Hex"])]
#[case::an_opponent_only_aura_on_an_opponent(&[(1, 1)], &[])]
fn the_state_based_actions_keep_an_aura_only_on_a_player_it_may_enchant(
    #[case] enchanting: &[(usize, usize)],
    #[case] gone: &[&str],
) {
    let mut game = game(enchanting);
    lose(&mut game, THIRD);
    let mut put: Vec<String> = state_based(&game, &[])
        .iter()
        .map(|event| match event {
            Event::ZoneChanged {
                object,
                to: Zone::Graveyard(owner),
                cause: ZoneCause::StateBased,
                ..
            } => {
                assert_eq!(*owner, ME);
                game.object(*object).unwrap().card().name.clone()
            }
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    // An Aura that is a creature may enchant nothing, wherever it starts.
    put.retain(|name| name != "Bestowed");
    put.sort();
    assert_eq!(put, gone);
}
