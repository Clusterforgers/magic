//! Goal: whole bot games with ward, prowess and battle cry in the decks (Monastery Swiftspear, Hulking Raptor, Hero of Bladehold, creatures of the three keywords, spells that give them for a turn, burn and removal that targets them, and a creature with an ability that targets) finish and keep every invariant (zone changes, life, the journal, the replay of the log equal to the live state) for many seeds at tables of two, three and four seats, and the games really use them: prowess, battle cry and ward triggers from printed and given keywords, wards that are paid and wards that are not, so that spells and abilities are countered, and the same games repeat from their seed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::abilities::AbilitySource;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::{Event, replay};
use mtg_testkit::{keyword_triggers_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(60)
}

#[derive(Default)]
struct Seen {
    prowess: usize,
    battle_cry: usize,
    ward: usize,
    given: usize,
    countered: usize,
    ordered: usize,
}

impl Seen {
    fn add(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::AbilityTriggered(trigger)
                    if matches!(
                        trigger.origin,
                        AbilitySource::PrintedKeyword(_) | AbilitySource::GrantedKeyword(_)
                    ) =>
                {
                    // What the event was about tells the keyword: a spell cast (a player and a spell), an
                    // attacker, or the target of a spell or ability (the permanent, what targets it, its controller).
                    match trigger.cause.subjects.len() {
                        1 => self.battle_cry += 1,
                        2 => self.prowess += 1,
                        _ => self.ward += 1,
                    }
                    if matches!(trigger.origin, AbilitySource::GrantedKeyword(_)) {
                        self.given += 1;
                    }
                }
                Event::ResolutionEnded {
                    outcome: ResolutionOutcome::Countered,
                    ..
                } => self.countered += 1,
                Event::TriggersOrdered { .. } => self.ordered += 1,
                _ => {}
            }
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_the_keywords_that_are_triggers_finish_and_keep_every_invariant(
    #[case] seats: usize,
) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = keyword_triggers_fuzz_setup(seats, seed).unwrap();
        let outcome =
            play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        seen.add(&outcome.events);
    }
    assert!(seen.prowess > 0, "prowess triggered");
    assert!(seen.battle_cry > 0, "battle cry triggered");
    assert!(seen.ward > 0, "ward triggered");
    assert!(seen.given > 0, "a keyword that was given triggered");
    assert!(
        seen.countered > 0,
        "a ward that was not paid countered something"
    );
    assert!(seen.ordered > 0, "triggers were put in order");
}

#[rstest]
#[case::two(2, 7)]
#[case::four(4, 8)]
fn a_game_of_the_keywords_repeats_exactly_from_its_seed(#[case] seats: usize, #[case] seed: u64) {
    let play = || {
        let setup = keyword_triggers_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    let setup = keyword_triggers_fuzz_setup(seats, seed).unwrap();
    let replayed = replay(&setup, &first.events).unwrap();
    assert_eq!(replayed.state_hash(), first.final_hash);
}
