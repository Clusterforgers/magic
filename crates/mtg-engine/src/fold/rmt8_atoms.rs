//! Goal: a creature attacks a battle only while the permanent it attacks is a battle (a planeswalker or a player is
//! another fact), an Aura that enchants a player is attached to that player and to no other, and a permanent fought
//! since a player's last upkeep only when it attacked, blocked or became blocked from that upkeep's turn on, at tables of
//! two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::apply_event;
use crate::combat::AttackTarget;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::turn::Step;

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

fn permanent(name: &str, kind: CardType) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(kind))
}

fn morph(name: &str) -> Arc<CardDefinition> {
    let rules: mtg_cards::ability::Ability =
        serde_json::from_str(r#"{"Keyword":{"Morph":"{1}{U}"}}"#).unwrap();
    Arc::new((*creature(name, 3, 3)).clone().with_rules(rules))
}

/// Seat 0 holds Bear, Shifter (with morph) and a Guest; seat 1 holds the battle Siege, the planeswalker Liliana and a
/// Guest; every other seat a Guest.
fn table(seats: usize) -> GameState {
    let field = |seat: usize| {
        let mut cards = vec![creature(&format!("Guest {seat}"), 2, 2)];
        if seat == 0 {
            cards.extend([creature("Bear", 2, 2), morph("Shifter")]);
        }
        if seat == 1 {
            cards.extend([
                permanent("Siege", CardType::Battle),
                permanent("Liliana", CardType::Planeswalker),
            ]);
        }
        cards
    };
    state(
        (0..seats).map(|_| Vec::new()).collect(),
        (0..seats).map(field).collect(),
    )
}

fn id(state: &GameState, name: &str) -> ObjectId {
    state
        .objects()
        .find(|o| o.whole_card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn holds(state: &GameState, pred: Pred, name: &str) -> bool {
    let object = state.object(id(state, name)).unwrap().reference();
    let source = state.object(id(state, "Guest 1")).unwrap().reference();
    Scope::announcing(state, FOE, source)
        .selects(&Selector(vec![pred]), object)
        .unwrap()
}

#[rstest]
#[case::the_battle("Siege", true)]
#[case::a_planeswalker("Liliana", false)]
#[case::the_player("foe", false)]
fn a_creature_attacks_a_battle_only_while_it_attacks_one(
    #[values(2, 3, 4)] seats: usize,
    #[case] at: &str,
    #[case] expected: bool,
) {
    let mut game = table(seats);
    let target = if at == "foe" {
        AttackTarget::Player(FOE)
    } else {
        AttackTarget::Planeswalker(id(&game, at))
    };
    let attackers = vec![(id(&game, "Bear"), target)];
    for event in [
        Event::StepBegan {
            step: Step::BeginCombat,
        },
        Event::AttackersDeclared {
            player: ME,
            attackers,
        },
    ] {
        apply_event(&mut game, &event).unwrap();
    }
    assert_eq!(holds(&game, Pred::AttackingBattle, "Bear"), expected);
    // A creature that does not attack attacks no battle.
    assert!(!holds(&game, Pred::AttackingBattle, "Shifter"));
}

#[rstest]
#[case::you(mtg_cards::refs::Ref::You, true)]
#[case::opponents(mtg_cards::refs::Ref::Opponents, false)]
#[case::everyone(mtg_cards::refs::Ref::EachPlayer, true)]
fn an_aura_on_a_player_is_attached_to_that_player(
    #[values(2, 3, 4)] seats: usize,
    #[case] who: mtg_cards::refs::Ref,
    #[case] expected: bool,
) {
    let mut game = table(seats);
    // Seat 1's Siege stands in for a Curse on seat 1, read by seat 1 ("you").
    let curse = id(&game, "Siege");
    game.objects
        .get_mut(&curse)
        .unwrap()
        .permanent
        .as_mut()
        .unwrap()
        .attached_player = Some(FOE);
    assert_eq!(
        holds(&game, Pred::AttachedTo(who.clone()), "Siege"),
        expected
    );
    // A permanent attached to nothing is attached to no player.
    assert!(!holds(&game, Pred::AttachedTo(who), "Liliana"));
}

/// Seat 0's Bear attacks seat 1 in turn 1 and seat 1's Guest blocks it.
fn fought(seats: usize) -> GameState {
    let mut game = table(seats);
    let (bear, guest) = (id(&game, "Bear"), id(&game, "Guest 1"));
    for event in [
        Event::StepBegan { step: Step::Upkeep },
        Event::StepBegan {
            step: Step::BeginCombat,
        },
        Event::AttackersDeclared {
            player: ME,
            attackers: vec![(bear, AttackTarget::Player(FOE))],
        },
        Event::BlockersDeclared {
            player: FOE,
            blocks: vec![(guest, bear)],
        },
    ] {
        apply_event(&mut game, &event).unwrap();
    }
    game
}

fn since(attacked: bool, blocked: bool, was_blocked: bool) -> Pred {
    Pred::FoughtSinceYourLastUpkeep {
        attacked,
        blocked,
        was_blocked,
    }
}

/// Whether the permanent fought since the last upkeep of seat 0 (the asker of `holds_for`).
fn holds_for_me(state: &GameState, pred: Pred, name: &str) -> bool {
    let object = state.object(id(state, name)).unwrap().reference();
    let source = state.object(id(state, "Guest 0")).unwrap().reference();
    Scope::announcing(state, ME, source)
        .selects(&Selector(vec![pred]), object)
        .unwrap()
}

#[rstest]
#[case::the_attacker_attacked("Bear", since(true, false, false), true)]
#[case::the_attacker_was_blocked("Bear", since(false, true, true), true)]
#[case::the_attacker_did_not_block("Bear", since(false, true, false), false)]
#[case::the_blocker_blocked("Guest 1", since(false, true, false), true)]
#[case::the_blocker_did_not_attack("Guest 1", since(true, false, false), false)]
#[case::a_bystander("Shifter", since(true, true, true), false)]
fn a_permanent_fought_since_the_last_upkeep(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] pred: Pred,
    #[case] expected: bool,
) {
    let mut game = fought(seats);
    assert_eq!(holds_for_me(&game, pred.clone(), name), expected);
    // Seat 0's next upkeep begins: the turn of the fight is still after the upkeep before it.
    let next = u32::try_from(seats).unwrap() + 1;
    for event in [
        Event::TurnBegan {
            number: next,
            active: ME,
            extra: false,
        },
        Event::StepBegan { step: Step::Upkeep },
    ] {
        apply_event(&mut game, &event).unwrap();
    }
    assert_eq!(holds_for_me(&game, pred.clone(), name), expected);
    // One more upkeep of seat 0 and the fight is older than the upkeep before the latest.
    for event in [
        Event::TurnBegan {
            number: next * 2,
            active: ME,
            extra: false,
        },
        Event::StepBegan { step: Step::Upkeep },
    ] {
        apply_event(&mut game, &event).unwrap();
    }
    assert!(!holds_for_me(&game, pred, name));
}
