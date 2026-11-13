//! Goal: a run folds its events as the design says: the lobby takes up to four heroes and starts only with a player
//! and every deck chosen; the first boss needs no vote; a lost fight costs a heart and is retried until the hearts
//! run out; a won fight carries each hero's life, brings Spirits back and offers loot; votes go to the most votes or
//! the torch bearer; bots answer every offer; and replaying the events gives the same run.
use mtg_campaign::run::{HEAL_AFTER_BOSS, HEARTS, LIFE, SPIRIT_RETURN};
use mtg_campaign::{Brain, Content, HeroDeck, Offer, Pick, Run, RunEvent, Status, content};
use proptest::prelude::*;
use rstest::rstest;

fn deck(colors: &str) -> HeroDeck {
    HeroDeck {
        id: format!("precon:{colors}"),
        name: format!("Deck {colors}"),
        colors: colors.to_owned(),
    }
}

/// A run whose lobby holds one player and `bots` bot allies, every deck chosen.
fn lobby(content: &Content, seed: u64, bots: usize) -> (Run, Vec<RunEvent>) {
    let mut events = vec![RunEvent::Joined {
        name: "Ana".into(),
        user: Some("u1".into()),
        bot: None,
    }];
    for n in 0..bots {
        events.push(RunEvent::Joined {
            name: format!("Bot {n}"),
            user: None,
            bot: Some(Brain::Normal),
        });
    }
    for hero in 0..=bots {
        events.push(RunEvent::DeckChosen {
            hero: u8::try_from(hero).unwrap(),
            deck: deck(["RG", "WU", "B", "WUBRG"][hero % 4]),
        });
    }
    let run = Run::replay(content, "warrens", seed, 0, &events).unwrap();
    (run, events)
}

fn apply(run: &mut Run, events: &mut Vec<RunEvent>, event: RunEvent) {
    let content = content().unwrap();
    run.apply(content, &event).unwrap();
    events.push(event);
}

#[test]
fn the_lobby_refuses_what_cannot_start() {
    let content = content().unwrap();
    let mut run = Run::new(content, "warrens", 1, 0).unwrap();
    assert!(run.apply(content, &RunEvent::Started).is_err(), "no player");
    run.apply(
        content,
        &RunEvent::Joined {
            name: "Ana".into(),
            user: Some("u1".into()),
            bot: None,
        },
    )
    .unwrap();
    let again = RunEvent::Joined {
        name: "Ana".into(),
        user: Some("u1".into()),
        bot: None,
    };
    assert!(run.apply(content, &again).is_err(), "joined twice");
    assert!(run.apply(content, &RunEvent::Started).is_err(), "no deck");
    for n in 0..3 {
        let bot = RunEvent::Joined {
            name: format!("Bot {n}"),
            user: None,
            bot: Some(Brain::Easy),
        };
        run.apply(content, &bot).unwrap();
    }
    let fifth = RunEvent::Joined {
        name: "Late".into(),
        user: None,
        bot: Some(Brain::Easy),
    };
    assert!(run.apply(content, &fifth).is_err(), "a fifth hero");
    assert_eq!(run.heroes.len(), 4);
    assert!(
        run.apply(content, &RunEvent::Voted { hero: 0, node: 0 })
            .is_err(),
        "no vote in the lobby"
    );
}

#[test]
fn the_first_boss_needs_no_vote() {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 3, 1);
    apply(&mut run, &mut events, RunEvent::Started);
    assert_eq!(run.status, Status::Fight { game: None });
    assert_eq!(run.at, Some(0));
    let plan = run.fight_plan(content).unwrap().unwrap();
    assert_eq!(plan.heroes.len(), 2);
    assert_eq!(plan.heroes[0].life, LIFE);
}

#[rstest]
#[case(1)]
#[case(2)]
#[case(3)]
fn each_lost_fight_costs_a_heart_until_defeat(#[case] losses: u8) {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 5, 1);
    apply(&mut run, &mut events, RunEvent::Started);
    for _ in 0..losses {
        apply(
            &mut run,
            &mut events,
            RunEvent::FightCreated { game: "g".into() },
        );
        let lost = RunEvent::FightEnded {
            won: false,
            lives: vec![0, 0],
            spirits: vec![0, 1],
        };
        apply(&mut run, &mut events, lost);
    }
    assert_eq!(run.hearts, HEARTS - losses);
    if losses == HEARTS {
        assert_eq!(run.status, Status::Defeat);
    } else {
        assert_eq!(
            run.status,
            Status::Fight { game: None },
            "the fight is retried"
        );
        assert!(
            run.heroes.iter().all(|h| h.life == LIFE / 2),
            "heroes regroup at half life"
        );
    }
}

#[test]
fn a_won_fight_carries_life_brings_spirits_back_and_offers_loot() {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 9, 2);
    apply(&mut run, &mut events, RunEvent::Started);
    apply(
        &mut run,
        &mut events,
        RunEvent::FightCreated { game: "g".into() },
    );
    let won = RunEvent::FightEnded {
        won: true,
        lives: vec![17, 0, 39],
        spirits: vec![1],
    };
    apply(&mut run, &mut events, won);
    assert_eq!(run.status, Status::Room);
    let lives: Vec<i32> = run.heroes.iter().map(|h| h.life).collect();
    assert_eq!(
        lives,
        vec![17 + HEAL_AFTER_BOSS, SPIRIT_RETURN + HEAL_AFTER_BOSS, LIFE]
    );
    for hero in &run.heroes {
        assert_eq!(hero.gold, 50);
        assert!(
            matches!(hero.offers.as_slice(), [Offer::Relics { relics }, Offer::Cards { .. }] if relics.len() == 3)
        );
    }
    // The bots answer; the player takes a relic and skips the card.
    let player_picks = [Pick::Take(1), Pick::Skip];
    for pick in player_picks {
        apply(&mut run, &mut events, RunEvent::Picked { hero: 0, pick });
    }
    while let Some(event) = run.bot_moves(content).into_iter().next() {
        apply(&mut run, &mut events, event);
    }
    assert_eq!(run.status, Status::Map);
    assert_eq!(run.heroes[0].relics.len(), 1);
    assert_eq!(run.heroes[0].gold, 60);
    assert_eq!(run.map.choices(run.at).len(), 2);
}

#[rstest]
#[case::majority(&[1, 1, 0], 1)]
#[case::tie_goes_to_the_torch(&[0, 1], 0)]
#[case::unanimous(&[1, 1], 1)]
fn votes_choose_the_next_camp(#[case] votes: &[usize], #[case] winner: usize) {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 11, votes.len() - 1);
    // Make every hero a player so each vote counts.
    for (at, hero) in run.heroes.iter_mut().enumerate() {
        hero.user = Some(format!("u{at}"));
        hero.bot = None;
    }
    apply(&mut run, &mut events, RunEvent::Started);
    apply(
        &mut run,
        &mut events,
        RunEvent::FightCreated { game: "g".into() },
    );
    let lives = vec![30; votes.len()];
    apply(
        &mut run,
        &mut events,
        RunEvent::FightEnded {
            won: true,
            lives,
            spirits: vec![],
        },
    );
    for hero in 0..votes.len() {
        for _ in 0..2 {
            let hero = u8::try_from(hero).unwrap();
            apply(
                &mut run,
                &mut events,
                RunEvent::Picked {
                    hero,
                    pick: Pick::Skip,
                },
            );
        }
    }
    let choices = run.map.choices(run.at);
    // The torch passed to hero 1 after the first boss; a tie goes to their vote.
    let torch_vote = votes.get(usize::from(run.torch())).copied();
    for (hero, vote) in votes.iter().enumerate() {
        let event = RunEvent::Voted {
            hero: u8::try_from(hero).unwrap(),
            node: choices[*vote],
        };
        apply(&mut run, &mut events, event);
    }
    let expected = if votes.iter().filter(|v| **v == 0).count() * 2 == votes.len() {
        choices[torch_vote.unwrap()]
    } else {
        choices[winner]
    };
    assert_eq!(run.at, Some(expected));
    assert!(run.votes.is_empty());
}

/// Plays a whole run: the party wins every fight, the player and the bots take the first choice of each offer.
fn play(seed: u64, bots: usize) -> (Run, Vec<RunEvent>) {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, seed, bots);
    apply(&mut run, &mut events, RunEvent::Started);
    let mut fights = 0;
    while !run.is_over() {
        match &run.status {
            Status::Fight { game: None } => {
                fights += 1;
                apply(
                    &mut run,
                    &mut events,
                    RunEvent::FightCreated {
                        game: format!("g{fights}"),
                    },
                );
            }
            Status::Fight { game: Some(_) } => {
                let lives = run.heroes.iter().map(|h| h.life - 5).collect();
                apply(
                    &mut run,
                    &mut events,
                    RunEvent::FightEnded {
                        won: true,
                        lives,
                        spirits: vec![],
                    },
                );
            }
            Status::Map if !run.votes.contains_key(&0) => {
                let node = run.map.choices(run.at)[0];
                apply(&mut run, &mut events, RunEvent::Voted { hero: 0, node });
            }
            Status::Room if !run.heroes[0].offers.is_empty() => {
                let pick = match &run.heroes[0].offers[0] {
                    Offer::Campfire => Pick::Rest,
                    Offer::Shop { .. } => Pick::Skip,
                    Offer::Cards { cards } if cards.is_empty() => Pick::Skip,
                    Offer::Relics { relics } if relics.is_empty() => Pick::Skip,
                    _ => Pick::Take(0),
                };
                apply(&mut run, &mut events, RunEvent::Picked { hero: 0, pick });
            }
            _ => {
                let moves = run.bot_moves(content);
                assert!(!moves.is_empty(), "stuck at {:?}", run.status);
                for event in moves {
                    apply(&mut run, &mut events, event);
                }
            }
        }
    }
    (run, events)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn a_run_won_fight_by_fight_ends_in_victory_and_replays(seed in any::<u64>(), bots in 0usize..4) {
        let content = content().unwrap();
        let (run, events) = play(seed, bots);
        prop_assert_eq!(&run.status, &Status::Victory);
        let bosses = run.map.nodes.iter().filter(|n| n.boss.is_some()).count();
        prop_assert_eq!(run.cleared.iter().filter(|id| run.map.node(**id).is_some_and(|n| n.boss.is_some())).count(), bosses);
        let replayed = Run::replay(content, "warrens", seed, 0, &events).unwrap();
        prop_assert_eq!(replayed, run);
    }
}
