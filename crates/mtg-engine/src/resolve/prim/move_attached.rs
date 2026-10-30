//! `Effect::Move` to `Destination::BattlefieldAttached`: cards put onto the battlefield attached to a named permanent.
//! Goal: an Aura enters attached only to a permanent its enchant ability allows (hexproof does not matter, nothing is
//! targeted; protection from Auras does) and stays where it was otherwise, an Equipment or a Fortification enters
//! attached when the permanent may hold it and unattached when it may not, a host that is gone leaves an Aura where it was
//! and lets the others in unattached, and the move binds a name again to the permanents that arrived.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::move_to::run;
use crate::choose::Target;
use crate::events::Event;
use crate::resolve::{Cx, Next, Resolving, Run, Scope};
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

fn card(name: &str, types: &[CardType], subtype: &str, rules: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name);
    for ty in types {
        card = card.with_type(*ty);
    }
    if !subtype.is_empty() {
        card.subtypes.push(Subtype(subtype.into()));
    }
    for json in rules {
        let ability: Ability = serde_json::from_str(json).unwrap();
        card = card.with_rules(ability);
    }
    Arc::new(card)
}

const ENCHANT_CREATURE: &str = r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}}"#;
const ENCHANT_LAND: &str = r#"{"Keyword":{"Enchant":[{"Type":"Land"}]}}"#;
const NO_AURAS: &str = r#"{"Keyword":{"Protection":[{"Subtype":"Aura"}]}}"#;
const HEXPROOF: &str = r#"{"Keyword":"Hexproof"}"#;

/// Seat 0 has a Bear, a Forest, a Warded creature (protection from Auras) and a Hexy creature (hexproof) on the
/// battlefield, and in the graveyard a Halo (Aura: creatures), a Rune (Aura: lands), a Blade (Equipment) and a Crusher
/// (Fortification).
fn game() -> GameState {
    let mut setup = crate::fixtures::setup(
        vec![vec![], vec![]],
        vec![
            vec![
                card("Bear", &[CardType::Creature], "", &[]),
                card("Forest", &[CardType::Land], "", &[]),
                card("Warded", &[CardType::Creature], "", &[NO_AURAS]),
                card("Hexy", &[CardType::Creature], "", &[HEXPROOF]),
            ],
            vec![],
        ],
    );
    setup.players[0].graveyard = vec![
        card(
            "Halo",
            &[CardType::Enchantment],
            "Aura",
            &[ENCHANT_CREATURE],
        ),
        card("Rune", &[CardType::Enchantment], "Aura", &[ENCHANT_LAND]),
        card("Blade", &[CardType::Artifact], "Equipment", &[]),
        card("Crusher", &[CardType::Artifact], "Fortification", &[]),
    ];
    GameState::from_setup(&setup).unwrap()
}

fn id(game: &GameState, name: &str) -> ObjectId {
    game.objects().find(|o| o.card().name == name).unwrap().id()
}

/// Runs the move of `Bound("a")`, the card named `mover`, onto the battlefield attached to `Bound("h")`, the permanent
/// named `host` (none when empty), and returns its events and the names it bound.
fn attached_move(
    game: &GameState,
    mover: &str,
    host: &str,
) -> (Vec<Event>, Vec<(String, Vec<Target>)>) {
    let source = game.object(id(game, "Bear")).unwrap();
    let mut resolving = Resolving::begin(source.id());
    let reference = |name: &str| Target::Object(game.object(id(game, name)).unwrap().reference());
    resolving
        .bindings
        .insert("a".into(), vec![reference(mover)]);
    resolving.bindings.insert(
        "h".into(),
        if host.is_empty() {
            Vec::new()
        } else {
            vec![reference(host)]
        },
    );
    let cx = Cx {
        scope: Scope {
            bindings: &resolving.bindings,
            ..Scope::announcing(game, ME, source.reference())
        },
        resolving: &resolving,
    };
    let effect: Effect = serde_json::from_str(
        r#"{"Move":{"what":{"Bound":"a"},"to":{"BattlefieldAttached":{"to":{"Bound":"h"}}}}}"#,
    )
    .unwrap();
    let Ok(Run::Done {
        events,
        bound,
        next,
        ..
    }) = run(&cx, &effect)
    else {
        panic!("the move is done");
    };
    assert_eq!(next, Next::Sibling);
    (events, bound)
}

#[rstest]
#[case::an_aura_on_a_creature("Halo", "Bear", true, true)]
#[case::an_aura_on_a_hexproof_creature("Halo", "Hexy", true, true)]
#[case::an_aura_on_a_land_it_cannot_enchant("Halo", "Forest", false, false)]
#[case::an_aura_of_lands_on_a_land("Rune", "Forest", true, true)]
#[case::an_aura_of_lands_on_a_creature("Rune", "Bear", false, false)]
#[case::an_aura_on_protection_from_auras("Halo", "Warded", false, false)]
#[case::an_aura_with_the_host_gone("Halo", "", false, false)]
#[case::an_equipment_on_a_creature("Blade", "Bear", true, true)]
#[case::an_equipment_on_protection_from_auras("Blade", "Warded", true, true)]
#[case::an_equipment_on_a_land("Blade", "Forest", true, false)]
#[case::an_equipment_with_the_host_gone("Blade", "", true, false)]
#[case::a_fortification_on_a_land("Crusher", "Forest", true, true)]
#[case::a_fortification_on_a_creature("Crusher", "Bear", true, false)]
fn a_card_enters_attached_when_the_host_may_hold_it(
    #[case] mover: &str,
    #[case] host: &str,
    #[case] enters: bool,
    #[case] attached: bool,
) {
    let game = game();
    let (events, _) = attached_move(&game, mover, host);
    let mover_id = id(&game, mover);
    let entered = events.iter().any(|event| {
        matches!(event, Event::ZoneChanged { object, to: Zone::Battlefield, .. } if *object == mover_id)
    });
    let attached_to = events.iter().find_map(|event| match event {
        Event::Attached { object, to } if *object == mover_id => Some(*to),
        _ => None,
    });
    assert_eq!(entered, enters, "{mover} on {host}");
    assert_eq!(
        attached_to,
        attached.then(|| id(&game, host)),
        "{mover} on {host}"
    );
}

#[test]
fn the_attachment_follows_its_card_into_the_batch() {
    let game = game();
    let (events, _) = attached_move(&game, "Halo", "Bear");
    let positions: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(at, event)| match event {
            Event::ZoneChanged { .. } | Event::Attached { .. } => Some(at),
            _ => None,
        })
        .collect();
    assert_eq!(positions.len(), 2, "one move and one attachment");
    assert!(matches!(events[positions[0]], Event::ZoneChanged { .. }));
    assert!(matches!(events[positions[1]], Event::Attached { .. }));
}

#[test]
fn a_name_is_bound_again_to_the_permanent_that_arrived() {
    let game = game();
    let (_, bound) = attached_move(&game, "Halo", "Bear");
    let [(name, targets)] = bound.as_slice() else {
        panic!("one name is bound again: {bound:?}");
    };
    assert_eq!(name, "a");
    let [Target::Object(arrived)] = targets.as_slice() else {
        panic!("one object arrived");
    };
    assert_eq!(arrived.id, id(&game, "Halo"));
    assert_eq!(
        arrived.counter,
        game.object(arrived.id).unwrap().counter() + 1
    );
}
