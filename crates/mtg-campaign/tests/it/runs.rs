//! Goal: a run folds its events as the design says: the lobby takes up to four heroes and starts only with a player
//! and every deck chosen; the first boss needs no vote; a lost fight costs a heart and is retried until the hearts
//! run out; a fight whose game ended unplayed waits for a new game at no cost; a fight whose table closed or that the host retreated from counts as lost; a won fight carries each hero's life, brings Spirits back and offers loot; votes go to the most votes or
//! the torch bearer; bots vote only after every player; a vote is always between two camps, a camp's next boss is
//! entered with no vote and stored votes for that boss still replay; bots answer every offer; the host chooses the timers in the lobby, every fight's game keeps them and a stored run without them is relaxed; each fight whose game
//! ended or lost its table is kept with its game for the fight log; and replaying the
//! events gives the same run.
use mtg_campaign::run::{HEAL_AFTER_BOSS, HEARTS, LIFE, SPIRIT_RETURN};
use mtg_campaign::{
    Brain, Content, Forfeit, HeroDeck, Offer, Pick, Run, RunEvent, Status, content,
};
use mtg_core::house::{HouseRules, Timers};
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
    for n in 0..5 {
        let bot = RunEvent::Joined {
            name: format!("Bot {n}"),
            user: None,
            bot: Some(Brain::Easy),
        };
        run.apply(content, &bot).unwrap();
    }
    let seventh = RunEvent::Joined {
        name: "Late".into(),
        user: None,
        bot: Some(Brain::Easy),
    };
    assert!(run.apply(content, &seventh).is_err(), "a seventh hero");
    assert_eq!(run.heroes.len(), 6);
    assert!(
        run.apply(content, &RunEvent::Voted { hero: 0, node: 0 })
            .is_err(),
        "no vote in the lobby"
    );
}

#[rstest]
#[case::relaxed(Timers::Relaxed)]
#[case::quick(Timers::Quick)]
#[case::off(Timers::Off)]
fn the_timers_are_chosen_in_the_lobby_and_every_fights_house_rules_keep_them(
    #[case] timers: Timers,
) {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 7, 1);
    assert_eq!(
        run.timers,
        Timers::Relaxed,
        "relaxed until the host chooses"
    );
    apply(&mut run, &mut events, RunEvent::Timers { timers });
    assert_eq!(run.timers, timers);
    apply(&mut run, &mut events, RunEvent::Started);
    assert_eq!(run.timers, timers, "starting keeps them");
    assert_eq!(
        Run::replay(content, "warrens", 7, 0, &events).unwrap(),
        run,
        "the events give the same run"
    );
    let plan = run.fight_plan(content).unwrap().unwrap();
    let house = plan.house_rules(&HouseRules {
        timers: Some(run.timers),
        ..HouseRules::default()
    });
    assert_eq!(house.timers, Some(timers), "the fight plays under them");
    assert!(house.teams.is_some(), "and keeps the fight's own rules");
}

#[test]
fn the_timers_cannot_change_once_the_run_has_started_and_a_run_without_the_event_is_relaxed() {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 7, 1);
    assert_eq!(
        Run::replay(content, "warrens", 7, 0, &events)
            .unwrap()
            .timers,
        Timers::Relaxed,
        "a stored run has no timers event"
    );
    apply(&mut run, &mut events, RunEvent::Started);
    let refused = run.apply(
        content,
        &RunEvent::Timers {
            timers: Timers::Off,
        },
    );
    assert!(refused.is_err(), "the run has started");
    assert_eq!(run.timers, Timers::Relaxed);
}

#[test]
fn a_run_stored_before_the_timers_read_as_relaxed() {
    let content = content().unwrap();
    let (run, _) = lobby(content, 7, 0);
    let mut stored = serde_json::to_value(&run).unwrap();
    stored.as_object_mut().unwrap().remove("timers");
    let back: Run = serde_json::from_value(stored).unwrap();
    assert_eq!(back.timers, Timers::Relaxed);
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
                let choices = run.map.choices(run.at);
                assert_eq!(choices.len(), 2, "a vote is always between two camps");
                let node = choices[0];
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

#[rstest]
#[case(1)]
#[case(3)]
fn a_voided_fight_waits_for_a_new_game_at_no_cost(#[case] voids: usize) {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 5, 1);
    apply(&mut run, &mut events, RunEvent::Started);
    assert!(
        run.apply(content, &RunEvent::FightVoided).is_err(),
        "no game to void yet"
    );
    let before = run.clone();
    for n in 0..voids {
        let game = format!("g{n}");
        apply(&mut run, &mut events, RunEvent::FightCreated { game });
        apply(&mut run, &mut events, RunEvent::FightVoided);
    }
    assert_eq!(run, before, "nothing but the game changed");
    assert_eq!(run.status, Status::Fight { game: None });
    let replayed = Run::replay(content, "warrens", 5, 0, &events).unwrap();
    assert_eq!(replayed, run);
}

#[rstest]
#[case::closed_table(Forfeit::Closed, true, "lost its table")]
#[case::host_retreat(Forfeit::Retreat, true, "retreated")]
#[case::retreat_before_a_game(Forfeit::Retreat, false, "retreated")]
fn a_forfeited_fight_costs_a_heart_like_a_lost_one_and_is_made_again(
    #[case] cause: Forfeit,
    #[case] with_game: bool,
    #[case] words: &str,
) {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 5, 1);
    apply(&mut run, &mut events, RunEvent::Started);
    if with_game {
        apply(
            &mut run,
            &mut events,
            RunEvent::FightCreated { game: "g".into() },
        );
    }
    apply(&mut run, &mut events, RunEvent::FightForfeited { cause });
    assert_eq!(run.hearts, HEARTS - 1);
    assert_eq!(run.status, Status::Fight { game: None });
    assert!(run.heroes.iter().all(|h| h.life == LIFE / 2));
    let line = run.log.last().unwrap();
    assert!(
        line.starts_with("The party") && line.contains(words),
        "{line}"
    );
    let replayed = Run::replay(content, "warrens", 5, 0, &events).unwrap();
    assert_eq!(replayed, run);
}

#[test]
fn each_finished_fight_is_kept_with_its_game_room_and_result() {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 5, 1);
    apply(&mut run, &mut events, RunEvent::Started);
    let room = run.at.unwrap();
    // A game that ended unplayed and a retreat before any game leave no fight to look back on.
    let created = |game: &str| RunEvent::FightCreated { game: game.into() };
    apply(&mut run, &mut events, created("void"));
    apply(&mut run, &mut events, RunEvent::FightVoided);
    let retreat = RunEvent::FightForfeited {
        cause: Forfeit::Retreat,
    };
    apply(&mut run, &mut events, retreat);
    assert_eq!(run.fights, Vec::new());
    apply(&mut run, &mut events, created("closed"));
    let closed = RunEvent::FightForfeited {
        cause: Forfeit::Closed,
    };
    apply(&mut run, &mut events, closed);
    apply(&mut run, &mut events, created("won"));
    let won = RunEvent::FightEnded {
        won: true,
        lives: vec![30; run.heroes.len()],
        spirits: Vec::new(),
    };
    apply(&mut run, &mut events, won);
    let kept: Vec<(&str, u8, bool)> = run
        .fights
        .iter()
        .map(|fight| (fight.game.as_str(), fight.room, fight.won))
        .collect();
    assert_eq!(kept, vec![("closed", room, false), ("won", room, true)]);
    let replayed = Run::replay(content, "warrens", 5, 0, &events).unwrap();
    assert_eq!(replayed, run);
}

#[test]
fn forfeiting_every_fight_ends_the_run_in_defeat() {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 5, 0);
    apply(&mut run, &mut events, RunEvent::Started);
    let retreat = RunEvent::FightForfeited {
        cause: Forfeit::Retreat,
    };
    for _ in 0..HEARTS {
        apply(&mut run, &mut events, retreat.clone());
    }
    assert_eq!(run.hearts, 0);
    assert_eq!(run.status, Status::Defeat);
    assert!(
        run.apply(content, &retreat).is_err(),
        "a finished run has no fight to retreat from"
    );
}

#[test]
fn a_table_that_never_was_cannot_close_and_the_lobby_is_no_fight_to_retreat_from() {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 5, 1);
    let retreat = RunEvent::FightForfeited {
        cause: Forfeit::Retreat,
    };
    assert!(
        run.apply(content, &retreat).is_err(),
        "the lobby is no fight"
    );
    apply(&mut run, &mut events, RunEvent::Started);
    let closed = RunEvent::FightForfeited {
        cause: Forfeit::Closed,
    };
    assert!(run.apply(content, &closed).is_err(), "no game to close yet");
    assert_eq!(run.hearts, HEARTS);
}

#[rstest]
#[case::alone(0)]
#[case::with_a_bot(1)]
#[case::with_three_bots(3)]
fn stored_votes_for_a_camps_next_boss_still_replay(#[case] bots: usize) {
    let content = content().unwrap();
    let (run, events) = play(21, bots);
    // Before a camp's next boss was entered on its own, every hero voted for it: such a log folds to the same run.
    let mut old = Vec::new();
    let mut step = Run::new(content, "warrens", 21, 0).unwrap();
    for event in &events {
        let before = step.status.clone();
        step.apply(content, event).unwrap();
        old.push(event.clone());
        if before == Status::Room && step.status == (Status::Fight { game: None }) {
            let node = step.at.unwrap();
            for hero in 0..step.heroes.len() {
                let hero = u8::try_from(hero).unwrap();
                let vote = RunEvent::Voted { hero, node };
                step.apply(content, &vote).unwrap();
                old.push(vote);
            }
        }
    }
    assert!(old.len() > events.len(), "the run passed a camp");
    assert_eq!(Run::replay(content, "warrens", 21, 0, &old).unwrap(), run);
}

#[rstest]
#[case::one_player_one_bot(1, 1)]
#[case::two_players_one_bot(2, 1)]
#[case::three_players_two_bots(3, 2)]
fn bots_vote_only_after_every_player(#[case] players: usize, #[case] bots: usize) {
    let content = content().unwrap();
    let (mut run, mut events) = lobby(content, 13, players + bots - 1);
    for (at, hero) in run.heroes.iter_mut().enumerate().take(players) {
        hero.user = Some(format!("u{at}"));
        hero.bot = None;
    }
    apply(&mut run, &mut events, RunEvent::Started);
    let game = RunEvent::FightCreated { game: "g".into() };
    apply(&mut run, &mut events, game);
    let won = RunEvent::FightEnded {
        won: true,
        lives: vec![30; players + bots],
        spirits: vec![],
    };
    apply(&mut run, &mut events, won);
    while run.status == Status::Room {
        let mut moves = run.bot_moves(content);
        for (hero, h) in run.heroes.iter().enumerate().take(players) {
            if !h.offers.is_empty() {
                let hero = u8::try_from(hero).unwrap();
                let pick = Pick::Skip;
                moves.push(RunEvent::Picked { hero, pick });
            }
        }
        for event in moves {
            apply(&mut run, &mut events, event);
        }
    }
    assert_eq!(run.status, Status::Map);
    let choices = run.map.choices(run.at);
    for hero in 0..players {
        assert!(
            run.bot_moves(content).is_empty(),
            "bots wait for the players"
        );
        let node = choices[hero % 2];
        let hero = u8::try_from(hero).unwrap();
        apply(&mut run, &mut events, RunEvent::Voted { hero, node });
    }
    let moves = run.bot_moves(content);
    assert_eq!(moves.len(), bots, "each bot votes once");
    for event in moves {
        apply(&mut run, &mut events, event);
    }
    assert_eq!(run.status, Status::Room, "the party entered a camp");
    assert!(choices.contains(&run.at.unwrap()));
}
