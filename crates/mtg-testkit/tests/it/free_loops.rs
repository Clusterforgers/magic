//! Goal: a bot game with free repeatable activations (an `{X}` look at X = 0, a free shuffle, a mana ability with a free untap that pays its cost back, a free loot that empties the library) ends without the watchdog at two to four seats and replays to its final state, a step of such abilities ends within a bound that the bare bot passes by far, and the guard still lets the bot repeat activations that change the game (a pump on each creature, mana for a spell).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::{Answer, PriorityAction};
use mtg_engine::format::Normal;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, Event, GameSetup, replay};
use mtg_testkit::bot::{STEP_REPEATS, apply_answer};
use mtg_testkit::cards::{bear, creature, forest, land};
use mtg_testkit::{PlayOptions, RandomBot, Scenario, play_game_with, ready};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Soothsaying's second ability: free at X = 0, and then it changes nothing.
const LOOK_AT_X: &str = r#"[{"Activated":{"cost":"{X}","effects":[{"LookAtTop":{"count":"X","as":"top"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Top","order":"Chosen"}}}}]}}]"#;
/// A free shuffle: it changes the library every time, so only the repeat bound ends it.
const SHUFFLE: &str = r#"[{"Activated":{"cost":"{0}","effects":[{"Shuffle":{}}]}}]"#;
/// A tap for mana and a free untap, which pays the tap back: a mana ability taken again and again. (A mana ability
/// that costs mana is used only inside a payment, never at priority.)
const UNTAP_MANA: &str = r#"[{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}},{"Activated":{"cost":"{0}","effects":[{"Untap":{"what":"This"}}]}}]"#;
/// A free loot: the bot draws its library empty with it.
const LOOT: &str = r#"[{"Activated":{"cost":"{0}","effects":[{"Draw":{}},{"Discard":{}}]}}]"#;
/// A free pump of the creature itself, which changes the game every time.
const PUMP: &str = r#"[{"Activated":{"cost":"{0}","effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}]"#;

/// Most answers a test plays of one step: the bare bot passes it, the guarded one stays far below.
const STEP_ANSWERS: usize = 3_000;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(4)
}

/// A card of `kind` for `{1}` with the abilities `rules`.
fn card(name: &str, kind: CardType, rules: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(kind)
        .with_mana_cost(ManaCost::parse("{1}").unwrap());
    for ability in serde_json::from_str::<Vec<Ability>>(rules).unwrap() {
        card = card.with_rules(ability);
    }
    if kind == CardType::Creature {
        card = card.with_power_toughness(1, 1);
    }
    Arc::new(card)
}

/// Sixty cards: four artifacts with `rules`, twelve bears and lands.
fn deck(rules: &str) -> Vec<Arc<CardDefinition>> {
    let relic = card("Free Relic", CardType::Artifact, rules);
    let mut deck = vec![relic; 4];
    for name in ["Bear A", "Bear B", "Bear C"] {
        deck.extend(std::iter::repeat_n(
            creature(name, 1, &[Color::Green], 2, 2),
            4,
        ));
    }
    deck.extend((0..44).map(|i| land([Color::Green, Color::Blue][i % 2])));
    deck
}

#[rstest]
fn free_activations_end_and_replay(
    #[values(LOOK_AT_X, SHUFFLE, UNTAP_MANA, LOOT)] rules: &str,
    #[values(2, 3, 4)] players: usize,
) {
    for_each_seed(0..games(), |seed| {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(rules); players]);
        let outcome = play_game_with(setup.clone(), seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"));
        let end = replay(&setup, &outcome.events).unwrap();
        assert_eq!(end.state_hash(), outcome.final_hash, "seed {seed}");
        // The games really use the abilities, again and again.
        let relic = |id: ObjectId| {
            end.object(id)
                .is_some_and(|o| o.card().name == "Free Relic")
        };
        let used = outcome
            .events
            .iter()
            .filter(|event| match event {
                Event::AbilityActivated { source, .. } => relic(source.id),
                Event::ManaAdded { source, .. } => source.is_some_and(relic),
                _ => false,
            })
            .count();
        assert!(used > 1, "seed {seed}: {used}");
        let again = play_game_with(setup, seed, PlayOptions::default()).unwrap();
        assert_eq!(again.final_hash, outcome.final_hash, "seed {seed}");
    });
}

/// The priority answers of seat 0's bot in the first main phase of a table where each seat holds four creatures with
/// `rules` and two Forests, and whether the step ended within [`STEP_ANSWERS`]; `guarded` shows the bot the game
/// ([`RandomBot::choose_in`]).
fn main_phase(
    players: usize,
    rules: &str,
    seed: u64,
    guarded: bool,
) -> (Vec<PriorityAction>, bool) {
    let relic = || ready(card("Free Relic", CardType::Creature, rules));
    let mut scenario = Scenario::new(players).at(2, 0, Step::Main1);
    for seat in 0..players {
        scenario = scenario.battlefield(
            seat,
            [
                relic(),
                relic(),
                relic(),
                relic(),
                ready(forest()),
                ready(forest()),
            ],
        );
    }
    let mut engine: Engine = scenario.hand(0, [bear()]).build().unwrap();
    let mut bots: Vec<RandomBot> = (0..players)
        .map(|seat| RandomBot::new(seed, seat as u64))
        .collect();
    let mut taken = Vec::new();
    for _ in 0..STEP_ANSWERS {
        if engine.state().turn().step() != Step::Main1 {
            return (taken, true);
        }
        let Some(decision) = engine.decision().cloned() else {
            return (taken, true);
        };
        let bot = &mut bots[decision.player.index()];
        let answer = if guarded {
            bot.choose_in(&decision, engine.state())
        } else {
            bot.choose(&decision)
        };
        if let (0, Answer::Priority(action)) = (decision.player.index(), &answer) {
            taken.push(action.clone());
        }
        apply_answer(&mut engine, &decision, answer).unwrap();
    }
    (taken, false)
}

/// How often seat 0 activated each of its abilities.
fn activations(taken: &[PriorityAction]) -> Vec<(PriorityAction, u32)> {
    let mut counts: Vec<(PriorityAction, u32)> = Vec::new();
    for action in taken.iter().filter(|action| {
        matches!(
            action,
            PriorityAction::Activate(_) | PriorityAction::ActivateMana(_)
        )
    }) {
        match counts.iter_mut().find(|(seen, _)| seen == action) {
            Some((_, n)) => *n += 1,
            None => counts.push((action.clone(), 1)),
        }
    }
    counts
}

#[rstest]
fn a_step_of_free_activations_ends_within_the_bound(
    #[values(LOOK_AT_X, SHUFFLE, UNTAP_MANA, LOOT)] rules: &str,
    #[values(2, 3, 4)] players: usize,
) {
    let mut bare = 0;
    for seed in 0..16 {
        let (taken, ended) = main_phase(players, rules, seed, true);
        let counts = activations(&taken);
        assert!(ended, "seed {seed}");
        assert!(
            counts.iter().all(|(_, n)| *n <= STEP_REPEATS),
            "seed {seed}: {counts:?}"
        );
        let (taken, _) = main_phase(players, rules, seed, false);
        bare += activations(&taken).iter().map(|(_, n)| *n).sum::<u32>();
    }
    // The bare bot takes them again and again: the loop the guard ends.
    assert!(bare > 16 * 4 * STEP_REPEATS, "{bare}");
}

#[rstest]
fn changing_activations_still_repeat(#[values(2, 3, 4)] players: usize) {
    let (mut sources, mut most, mut cast_with_floating) = (Vec::new(), 0, false);
    for seed in 0..32 {
        let (taken, ended) = main_phase(players, PUMP, seed, true);
        assert!(ended, "seed {seed}");
        for (action, n) in activations(&taken) {
            if let PriorityAction::Activate(ability) = action
                && !sources.contains(&ability.object)
            {
                sources.push(ability.object);
            }
            most = most.max(n);
        }
        // Mana made by a mana ability at priority is spent on a spell.
        let tapped = taken
            .iter()
            .position(|a| matches!(a, PriorityAction::ActivateMana(_)));
        let cast = taken
            .iter()
            .rposition(|a| matches!(a, PriorityAction::CastSpell(_)));
        cast_with_floating |= matches!((tapped, cast), (Some(t), Some(c)) if t < c);
    }
    // Every pump is taken, and taken again up to the bound since each one changes the creature.
    assert_eq!(sources.len(), 4);
    assert_eq!(most, STEP_REPEATS);
    assert!(cast_with_floating);
}
