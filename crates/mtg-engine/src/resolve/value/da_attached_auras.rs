//! Goal: the selector "all Auras you control attached to it" (the Auras of the controller whose host is the first target) names the Auras of its controller that are attached to the target, and no Aura of another controller on it, no Aura on another permanent, no Equipment, and nothing once the target has left the battlefield; it follows an Aura that is attached to another host.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::refs::{Bounds, Selector, TargetSpec, Targetable};
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::Scope;
use crate::choose::{Choices, Target};
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, setup};
use crate::fold::apply_event;
use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;
use crate::zones::Zone;

const MY_AURAS_ON_IT: &str =
    r#"[{"Subtype":"Aura"},{"ControlledBy":"You"},{"AttachedTo":{"Target":0}}]"#;

fn attachment(name: &str, kind: &str, card_type: CardType) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(card_type)
            .with_subtype(kind),
    )
}

/// Seat 0 controls a Bear with two Auras and an Equipment on it, a Cub with one Aura and a Hex Aura on the Guard;
/// seat 1 controls the Guard, an Aura of its own on it and an Aura on the Bear.
fn board() -> GameState {
    let aura = |name: &str| attachment(name, "Aura", CardType::Enchantment);
    let mut game = setup(
        vec![vec![], vec![]],
        vec![
            vec![
                creature("Bear", 2, 2),
                creature("Cub", 1, 1),
                aura("First Aura"),
                aura("Second Aura"),
                attachment("Blade", "Equipment", CardType::Artifact),
                aura("Cub Aura"),
                aura("Hex Aura"),
            ],
            vec![
                creature("Guard", 3, 3),
                aura("Foe Aura"),
                aura("Guard Aura"),
            ],
        ],
    );
    let on =
        |game: &mut crate::setup::GameSetup, seat: usize, place: usize, host: (usize, usize)| {
            game.players[seat].battlefield[place].attached_to = Some(host);
        };
    on(&mut game, 0, 2, (0, 0));
    on(&mut game, 0, 3, (0, 0));
    on(&mut game, 0, 4, (0, 0));
    on(&mut game, 0, 5, (0, 1));
    on(&mut game, 0, 6, (1, 0));
    on(&mut game, 1, 1, (0, 0));
    on(&mut game, 1, 2, (1, 0));
    GameState::from_setup(&game).unwrap()
}

fn named(game: &GameState, name: &str) -> ObjectRef {
    game.objects()
        .find(|object| object.card().name == name)
        .map(GameObject::reference)
        .unwrap()
}

/// The Auras the selector names for seat 0 when `host` is the first target, as sorted names.
fn matched(game: &GameState, host: &str, selector: &str) -> Vec<String> {
    let target = Target::Object(named(game, host));
    let choices = Choices {
        targets: vec![vec![target]],
        ..Choices::default()
    };
    let specs = [TargetSpec {
        what: Targetable::Object(Selector(vec![mtg_cards::refs::Pred::Type(
            CardType::Creature,
        )])),
        bounds: Bounds::default(),
        divide: None,
    }];
    let mut scope = Scope::announcing(game, PlayerId(0), named(game, "Bear"));
    scope.choices = &choices;
    scope.specs = specs.iter().collect();
    let selector: Selector = serde_json::from_str(selector).unwrap();
    let mut found: Vec<String> = scope
        .matching(&selector)
        .unwrap()
        .into_iter()
        .map(|found| game.object(found.id).unwrap().card().name.clone())
        .collect();
    found.sort();
    found
}

#[rstest]
#[case::two_auras_and_not_the_equipment("Bear", &["First Aura", "Second Aura"])]
#[case::one_aura("Cub", &["Cub Aura"])]
#[case::the_aura_of_seat_0_on_a_creature_of_seat_1("Guard", &["Hex Aura"])]
fn the_auras_of_the_controller_attached_to_the_target_are_named(
    #[case] host: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(matched(&board(), host, MY_AURAS_ON_IT), expected);
}

#[rstest]
#[case::any_controller(r#"[{"Subtype":"Aura"},{"AttachedTo":{"Target":0}}]"#, "Bear", &["First Aura", "Foe Aura", "Second Aura"])]
#[case::an_opponents_aura(r#"[{"Subtype":"Aura"},{"ControlledBy":"Opponents"},{"AttachedTo":{"Target":0}}]"#, "Bear", &["Foe Aura"])]
#[case::equipment(r#"[{"Subtype":"Equipment"},{"AttachedTo":{"Target":0}}]"#, "Bear", &["Blade"])]
#[case::anything_attached(r#"[{"AttachedTo":{"Target":0}}]"#, "Cub", &["Cub Aura"])]
fn the_selector_tells_controllers_and_kinds_apart(
    #[case] selector: &str,
    #[case] host: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(matched(&board(), host, selector), expected);
}

#[test]
fn an_aura_that_is_attached_to_another_host_is_named_for_that_host_only() {
    let mut game = board();
    let (aura, cub) = (named(&game, "First Aura").id, named(&game, "Cub").id);
    apply_event(
        &mut game,
        &Event::Attached {
            object: aura,
            to: cub,
        },
    )
    .unwrap();
    assert_eq!(matched(&game, "Bear", MY_AURAS_ON_IT), ["Second Aura"]);
    assert_eq!(
        matched(&game, "Cub", MY_AURAS_ON_IT),
        ["Cub Aura", "First Aura"]
    );
}

#[test]
fn nothing_is_attached_to_a_target_that_has_left_the_battlefield() {
    let mut game = board();
    let bear = named(&game, "Bear");
    apply_event(
        &mut game,
        &Event::ZoneChanged {
            object: bear.id,
            from: Zone::Battlefield,
            to: Zone::Hand(PlayerId(0)),
            cause: ZoneCause::Moved,
        },
    )
    .unwrap();
    // The Auras fall off with their host (a state-based action), and the target is gone: the selector names no Aura.
    assert_eq!(matched(&game, "Bear", MY_AURAS_ON_IT), Vec::<String>::new());
}
