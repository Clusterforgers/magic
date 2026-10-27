//! Goal: however many permanents the table holds, the layers are worked out once for all the reads between two events (the options of a priority round, an attack and a block, the snapshots of a sweep), and a block offers every blocker every attacker it may block.
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::tests::rules;
use super::*;
use crate::combat::{AttackTarget, attack_options, block_options};
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::priority::priority_options;
use crate::snapshot::snapshot;
use crate::turn::Step;

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

/// An enchantment that gives every creature +1/+1, so the layers have something to work out.
fn anthem() -> Arc<CardDefinition> {
    let ability = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
    Arc::new(
        CardDefinition::new("Anthem")
            .with_type(CardType::Enchantment)
            .with_rules(rules(ability)),
    )
}

/// An anthem and `size` creatures for seat 0, `size` creatures for seat 1; the game stands at the declaration of blockers
/// after seat 0 attacked with all of its creatures.
fn crowd(size: usize) -> GameState {
    let mut mine = vec![anthem()];
    mine.extend((0..size).map(|n| creature(&format!("Crowd {n}"), 1, 1)));
    let theirs = (0..size)
        .map(|n| creature(&format!("Foe {n}"), 0, 5))
        .collect();
    let mut game = state(vec![vec![], vec![]], vec![mine, theirs]);
    let attackers: Vec<_> = game
        .objects()
        .filter(|object| object.card().name.starts_with("Crowd"))
        .map(|object| (object.id(), AttackTarget::Player(FOE)))
        .collect();
    let declared = Event::AttackersDeclared {
        player: ME,
        attackers,
    };
    apply_event(&mut game, &declared).unwrap();
    game.turn.step = Step::DeclareBlockers;
    game
}

/// How many times the layers were worked out while `read` ran.
fn computed(read: impl FnOnce()) -> u64 {
    COMPUTED.with(|count| count.set(0));
    read();
    COMPUTED.with(Cell::get)
}

#[rstest]
fn a_block_works_out_the_layers_once_and_offers_every_pair(#[values(30, 120)] size: usize) {
    let game = crowd(size);
    let mut options = None;
    let count = computed(|| options = Some(block_options(&game, FOE)));
    assert_eq!(count, 1);
    let options = options.unwrap();
    assert_eq!(options.blockers.len(), size);
    assert!(options.blockers.iter().all(|b| b.can_block.len() == size));
}

#[rstest]
fn the_options_of_a_round_work_out_the_layers_once(#[values(30, 120)] size: usize) {
    let game = crowd(size);
    let count = computed(|| {
        let _ = (priority_options(&game, ME), priority_options(&game, FOE));
        let _ = (attack_options(&game, ME), block_options(&game, FOE));
    });
    assert_eq!(count, 1);
}

#[rstest]
fn the_snapshots_of_a_sweep_work_out_the_layers_once(#[values(30, 120)] size: usize) {
    let game = crowd(size);
    let everything: Vec<_> = game.zone(crate::zones::Zone::Battlefield).to_vec();
    let mut shots = 0;
    let count = computed(|| {
        shots = everything
            .iter()
            .filter_map(|id| snapshot(&game, *id))
            .count();
    });
    assert_eq!((count, shots), (1, 2 * size + 1));
}
