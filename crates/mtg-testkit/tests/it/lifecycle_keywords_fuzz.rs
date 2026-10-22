//! Goal: whole bot games at two, three and four seats with decks of vanishing, fading and backup cards (the corpus cards of the three keywords and burn) finish and keep every invariant (zone changes, life, counters, the journal, the replay of the log equal to the live state), and the games really use the keywords: time and fade counters put on and taken off, permanents sacrificed by them, backup triggers that put +1/+1 counters, and the same games repeat from their seed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::seeds::fuzz_games;

use mtg_cards::value::CounterKind;
use mtg_engine::abilities::AbilitySource;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, replay};
use mtg_testkit::lifecycle_cards::{BACKUP_CARDS, FADING_CARDS, VANISHING_CARDS};
use mtg_testkit::{lifecycle_fuzz_setup, play_game};
use rstest::rstest;

/// Games per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(40)
}

fn listed(list: &[(&str, usize)], name: &str) -> bool {
    list.iter().any(|(card, _)| *card == name)
}

#[derive(Default)]
struct Seen {
    time_put: usize,
    time_removed: usize,
    fade_put: usize,
    fade_removed: usize,
    countdown_sacrifices: usize,
    backup_triggers: usize,
    backup_counters: usize,
}

impl Seen {
    fn add(&mut self, setup: &mtg_engine::GameSetup, events: &[Event]) {
        let engine = Engine::new(setup.clone()).unwrap();
        let name = |id| {
            engine
                .state()
                .object(id)
                .map(|object| object.card().name.clone())
                .unwrap_or_default()
        };
        let mut backup_sources = Vec::new();
        for event in events {
            match event {
                Event::AbilityTriggered(trigger) => {
                    let source = name(trigger.source.id);
                    if matches!(trigger.origin, AbilitySource::PrintedKeyword(_))
                        && listed(BACKUP_CARDS, &source)
                    {
                        self.backup_triggers += 1;
                        backup_sources.push(trigger.source.id);
                    }
                }
                Event::ZoneChanged {
                    object,
                    from: Zone::Battlefield,
                    to: Zone::Graveyard(_),
                    ..
                } => {
                    let card = name(*object);
                    if listed(VANISHING_CARDS, &card) || listed(FADING_CARDS, &card) {
                        self.countdown_sacrifices += 1;
                    }
                }
                Event::CountersChanged(change) => match (&change.counter, change.delta) {
                    (CounterKind::Other(kind), delta) if kind == "time" => {
                        if delta > 0 {
                            self.time_put += 1;
                        } else {
                            self.time_removed += 1;
                        }
                    }
                    (CounterKind::Other(kind), delta) if kind == "fade" => {
                        if delta > 0 {
                            self.fade_put += 1;
                        } else {
                            self.fade_removed += 1;
                        }
                    }
                    (CounterKind::PlusOnePlusOne, delta) if delta > 0 => {
                        self.backup_counters += usize::from(!backup_sources.is_empty());
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_vanishing_fading_and_backup_finish_and_keep_every_invariant(
    #[case] seats: usize,
) {
    let mut seen = Seen::default();
    for seed in 0..games() {
        let setup = lifecycle_fuzz_setup(seats, seed).unwrap();
        let outcome = play_game(setup.clone(), seed)
            .unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
        seen.add(&setup, &outcome.events);
    }
    assert!(seen.time_put > 0, "permanents entered with time counters");
    assert!(seen.time_removed > 0, "time counters were removed");
    assert!(seen.fade_put > 0, "permanents entered with fade counters");
    assert!(seen.fade_removed > 0, "fade counters were removed");
    assert!(seen.countdown_sacrifices > 0, "a permanent faded away");
    assert!(
        seen.backup_triggers > 0,
        "a backup trigger went on the stack"
    );
    assert!(seen.backup_counters > 0, "backup put +1/+1 counters");
}

#[rstest]
#[case::two(2, 21)]
#[case::four(4, 22)]
fn a_game_of_the_lifecycle_keywords_repeats_exactly_from_its_seed(
    #[case] seats: usize,
    #[case] seed: u64,
) {
    let play = || {
        let setup = lifecycle_fuzz_setup(seats, seed).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"))
    };
    let (first, second) = (play(), play());
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    let setup = lifecycle_fuzz_setup(seats, seed).unwrap();
    let replayed = replay(&setup, &first.events).unwrap();
    assert_eq!(replayed.state_hash(), first.final_hash);
}
