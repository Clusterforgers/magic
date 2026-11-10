//! Goal: "copy that spell" copies a spell that has left the stack by last known information (CR 608.2h, 707.10), at two, three and four seats: with two Krarks a lost flip that returns the spell to its owner's hand leaves the other Krark's won flip a copy of it, whatever the order of the flips, and the same holds for any "copy that spell" trigger that resolves after the spell was returned; the spell itself resolves only when nothing returned it, and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::rng::GameRng;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Krark, the Thumbless's trigger as the cards hold it: a won flip copies the spell, a lost one returns it to its
/// owner's hand.
const KRARK: &str = r#"{"Triggered":{"when":{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}},"effects":[{"FlipCoin":{"win":[{"CopySpell":{"what":"Triggering","new_targets":true}}],"lose":[{"Move":{"what":"Triggering","to":"Hand"}}]}}]}}"#;
const COPIER: &str = r#"{"Triggered":{"when":{"SpellCast":{"by":"You","what":[{"Type":"Sorcery"}]}},"effects":[{"CopySpell":{"what":"Triggering"}}]}}"#;
const RETURNER: &str = r#"{"Triggered":{"when":{"SpellCast":{"by":"You","what":[{"Type":"Sorcery"}]}},"effects":[{"Move":{"what":"Triggering","to":"Hand"}}]}}"#;
const BOON: &str = r#"{"Spell":{"effects":[{"GainLife":{"amount":3}}]}}"#;

fn creature(name: &str, rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(serde_json::from_str(rules).unwrap()),
    )
}

fn boon() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Boon")
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_type(CardType::Sorcery)
            .with_rules(serde_json::from_str(BOON).unwrap()),
    )
}

/// The first seed whose stream, from the substream `index` on, wins and loses the flips as `wins` says.
fn seed_for(index: u64, wins: &[bool]) -> u64 {
    (0u64..1_000_000)
        .find(|seed| {
            wins.iter()
                .zip(index..)
                .all(|(win, at)| (GameRng::substream(*seed, at).below(2) == 0) == *win)
        })
        .unwrap()
}

/// Casts Boon with seat 0's `permanents` out, puts the two triggers on the stack with the one of the first permanent on
/// top when `first_on_top` (it resolves first) and resolves everything, answering every other question passively.
fn cast_and_resolve(
    seats: usize,
    seed: u64,
    permanents: [Arc<CardDefinition>; 2],
    first_on_top: bool,
) -> Driver {
    let first = permanents[0].name.clone();
    let scenario = Scenario::new(seats)
        .seed(seed)
        .battlefield(0, permanents.map(ready))
        .hand(0, [boon()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Boon").unwrap();
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::OrderTriggers(options) => {
                let state = driver.state();
                let named_first = |at: usize| {
                    state
                        .object(options.triggers[at].source.id)
                        .unwrap()
                        .card()
                        .name
                        == first
                };
                let top = usize::from(named_first(0) != first_on_top);
                driver.order_triggers(&[1 - top, top]).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    driver
}

fn copies(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::SpellCopied(_)))
        .count()
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn in_hand(driver: &Driver) -> bool {
    driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .iter()
        .any(|id| driver.state().object(*id).unwrap().card().name == "Boon")
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Two Krarks flip in turn: each won flip copies Boon, a lost one sends it back to the hand. A loss before a win still
/// leaves the win its copy.
#[rstest]
fn two_krarks_copy_the_spell_whatever_the_order_of_the_flips(
    #[values(2, 3, 4)] seats: usize,
    #[values([true, true], [false, true], [true, false], [false, false])] flips: [bool; 2],
) -> Result<(), ScriptError> {
    let start = Driver::from_scenario(Scenario::new(seats))?
        .state()
        .rng_index();
    let driver = cast_and_resolve(
        seats,
        seed_for(start, &flips),
        [creature("Krark A", KRARK), creature("Krark B", KRARK)],
        true,
    );
    let wins = flips.iter().filter(|won| **won).count();
    let lost = flips.contains(&false);
    assert_eq!(copies(&driver), wins, "one copy for each won flip");
    assert_eq!(in_hand(&driver), lost, "a lost flip returns the spell");
    let resolutions = wins + usize::from(!lost);
    assert_eq!(life(&driver), 20 + 3 * i32::try_from(resolutions).unwrap());
    replays(&driver);
    Ok(())
}

/// A "copy that spell" trigger copies the spell whether it resolves before or after another trigger returned the spell to
/// its owner's hand.
#[rstest]
fn a_copy_trigger_copies_a_spell_returned_before_it_resolves(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] copier_first: bool,
) {
    let permanents = [creature("Copier", COPIER), creature("Returner", RETURNER)];
    let driver = cast_and_resolve(seats, 0, permanents, copier_first);
    let copied_first = driver
        .engine()
        .events()
        .iter()
        .find_map(|event| match event {
            Event::SpellCopied(_) => Some(true),
            Event::ZoneChanged {
                to: Zone::Hand(_), ..
            } => Some(false),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        copied_first, copier_first,
        "the triggers resolve in the order set"
    );
    assert_eq!(copies(&driver), 1, "the copy is made either way");
    assert!(in_hand(&driver));
    assert_eq!(
        life(&driver),
        23,
        "the copy resolves, the returned spell does not"
    );
    replays(&driver);
}
