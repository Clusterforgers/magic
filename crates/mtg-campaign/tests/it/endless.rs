//! Goal: endless Delves holds together: a cleared floor of the endless generation waits for the party's vote to bank
//! the win or descend (most votes, a tie to the torch bearer; bots descend only with two hearts and 60 % life); a
//! deeper floor's ladder is drawn by seed and floor, avoids the bosses just fought and ends in one of the theme's
//! final bosses; runs of older generations end in victory as before.
use mtg_campaign::map::{LIEUTENANTS, generate_floor};
use mtg_campaign::run::{CONTENT, ENDLESS_FROM, HEARTS};
use mtg_campaign::{Brain, HeroDeck, Run, RunEvent, Status, content};
use proptest::prelude::*;
use rstest::rstest;

pub(crate) fn deck() -> HeroDeck {
    HeroDeck {
        id: "x".into(),
        name: "x".into(),
        colors: "G".into(),
        commanders: Vec::new(),
        makeup: None,
    }
}

/// A started run of generation `version` with `players` players and `bots` bot allies.
pub(crate) fn started(seed: u64, heat: u8, version: u32, players: usize, bots: usize) -> Run {
    let mut events = vec![RunEvent::Content { version }];
    for at in 0..players + bots {
        let bot = at >= players;
        events.push(RunEvent::Joined {
            name: format!("H{at}"),
            user: (!bot).then(|| format!("u{at}")),
            bot: bot.then_some(Brain::Normal),
        });
        events.push(RunEvent::DeckChosen {
            hero: u8::try_from(at).unwrap(),
            deck: deck(),
        });
    }
    events.push(RunEvent::Started);
    Run::replay(content().unwrap(), "warrens", seed, heat, &events).unwrap()
}

/// Wins every fight of the floor, every hero answering like a bot, until the floor is cleared or the run won.
pub(crate) fn clear_floor(run: &mut Run) {
    let content = content().unwrap();
    for _ in 0..200 {
        let event = match &run.status {
            Status::Fight { game: None } => RunEvent::FightCreated {
                game: format!("g{}", run.fights.len()),
            },
            Status::Fight { game: Some(_) } => RunEvent::FightEnded {
                won: true,
                lives: run.heroes.iter().map(|h| h.life).collect(),
                spirits: Vec::new(),
            },
            Status::Map | Status::Room => {
                let mut as_bots = run.clone();
                for h in &mut as_bots.heroes {
                    h.bot = Some(Brain::Normal);
                    h.user = None;
                }
                for event in as_bots.bot_moves(content) {
                    run.apply(content, &event).unwrap();
                }
                continue;
            }
            _ => return,
        };
        run.apply(content, &event).unwrap();
    }
    panic!("the floor never ended: {:?}", run.status);
}

pub(crate) fn vote(run: &mut Run, hero: usize, descend: bool) {
    let event = RunEvent::FloorVoted {
        hero: u8::try_from(hero).unwrap(),
        descend,
    };
    run.apply(content().unwrap(), &event).unwrap();
}

fn bosses(run: &Run) -> Vec<String> {
    run.map
        .nodes
        .iter()
        .filter_map(|n| n.boss.clone())
        .collect()
}

#[rstest]
#[case::most_descend(3, &[true, true, false], Some(true))]
#[case::most_bank(3, &[false, true, false], Some(false))]
#[case::tie_to_the_torch(2, &[true, false], None)]
#[case::tie_to_the_torch_the_other_way(2, &[false, true], None)]
fn a_cleared_floor_votes_to_bank_or_descend(
    #[case] players: usize,
    #[case] votes: &[bool],
    #[case] expected: Option<bool>,
) {
    let mut run = started(5, 1, 6, players, 0);
    clear_floor(&mut run);
    assert_eq!(run.status, Status::FloorCleared);
    assert_eq!((run.floor, run.floors_cleared), (1, 1));
    let descend = expected.unwrap_or(votes[usize::from(run.torch())]);
    let refused = RunEvent::Voted { hero: 0, node: 0 };
    assert!(run.apply(content().unwrap(), &refused).is_err());
    for (hero, d) in votes.iter().enumerate() {
        assert_eq!(run.status, Status::FloorCleared, "waits for every hero");
        vote(&mut run, hero, *d);
    }
    if descend {
        assert_eq!(run.floor, 2);
        assert_eq!(run.status, Status::Fight { game: None });
    } else {
        assert_eq!(run.status, Status::Victory);
        assert_eq!(run.floor, 1);
    }
    let late = RunEvent::FloorVoted {
        hero: 0,
        descend: true,
    };
    assert!(run.apply(content().unwrap(), &late).is_err(), "no vote now");
}

#[test]
fn a_descent_keeps_the_party_and_gives_a_heart_back() {
    let content = content().unwrap();
    let mut run = started(9, 1, 6, 1, 1);
    clear_floor(&mut run);
    run.hearts = 1;
    run.heroes[0].life = 23;
    run.heroes[0].relics.push("phoenix_feather".into());
    run.heroes[1].satchel.push("Llanowar Elves".into());
    let before = run.clone();
    let fought = bosses(&run);
    vote(&mut run, 0, true);
    vote(&mut run, 1, true);
    assert_eq!(run.hearts, 2, "one heart back");
    assert_eq!(
        run.heroes, before.heroes,
        "life, relics, Satchel and upgrades stay"
    );
    let avoid: Vec<&str> = fought.iter().map(String::as_str).collect();
    let theme = &content.themes[0];
    assert_eq!(run.map, generate_floor(theme, 9, 6, 2, &avoid));
    assert_eq!(run.cleared, Vec::<u8>::new());
    assert_eq!(run.at, run.map.choices(None).first().copied());
    // A full party gets no heart above the start.
    let mut full = before.clone();
    full.hearts = HEARTS;
    vote(&mut full, 0, true);
    vote(&mut full, 1, true);
    assert_eq!(full.hearts, HEARTS);
    // The second floor clears into the same choice, and the chronicle and summary count both floors.
    clear_floor(&mut run);
    assert_eq!(
        (run.status.clone(), run.floors_cleared),
        (Status::FloorCleared, 2)
    );
    assert!(run.fights.iter().any(|f| f.floor == 2 && f.won));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn a_deeper_floor_is_drawn_by_seed_and_avoids_the_bosses_just_fought(seed in any::<u64>(), floor in 2u8..6) {
        let theme = &content().unwrap().themes[0];
        let above = generate_floor(theme, seed, CONTENT, floor - 1, &[]);
        let fought: Vec<&str> = above.nodes.iter().filter_map(|n| n.boss.as_deref()).collect();
        let map = generate_floor(theme, seed, CONTENT, floor, &fought);
        prop_assert_eq!(&map, &generate_floor(theme, seed, CONTENT, floor, &fought));
        let ids: Vec<&str> = map.nodes.iter().filter_map(|n| n.boss.as_deref()).collect();
        prop_assert_eq!(ids.len(), LIEUTENANTS + 1);
        prop_assert!(theme.boss(ids[LIEUTENANTS]).unwrap().final_boss);
        let fresh = theme.bosses.iter().filter(|b| !b.final_boss && b.since <= CONTENT && !fought.contains(&b.id.as_str())).count();
        let repeats = ids[..LIEUTENANTS].iter().filter(|id| fought.contains(id)).count();
        prop_assert_eq!(repeats, LIEUTENANTS.saturating_sub(fresh));
    }
}

#[rstest]
fn a_ladder_ends_in_either_final_boss(#[values(1, 2, 3)] floor: u8) {
    let theme = &content().unwrap().themes[0];
    let finals: std::collections::BTreeSet<String> = (0..64u64)
        .filter_map(|seed| {
            generate_floor(theme, seed, CONTENT, floor, &[])
                .last()?
                .boss
                .clone()
        })
        .collect();
    let expected: std::collections::BTreeSet<String> =
        ["grimgut_court", "unmaker_void"].map(str::to_owned).into();
    assert_eq!(finals, expected);
}

#[test]
fn floors_differ_by_seed_and_floor() {
    let theme = &content().unwrap().themes[0];
    let ladder = |seed: u64, floor: u8| {
        let map = generate_floor(theme, seed, CONTENT, floor, &[]);
        map.nodes
            .iter()
            .map(|n| (n.kind, n.boss.clone()))
            .collect::<Vec<_>>()
    };
    let by_floor = (1..=8)
        .map(|f| ladder(3, f))
        .collect::<std::collections::BTreeSet<_>>();
    let by_seed = (1..=8)
        .map(|s| ladder(s, 2))
        .collect::<std::collections::BTreeSet<_>>();
    assert!(by_floor.len() > 1 && by_seed.len() > 1);
}

#[rstest]
fn older_runs_end_in_victory_as_before(#[values(0, 3, 4)] version: u32) {
    assert!(version < ENDLESS_FROM);
    let mut run = started(7, 1, version, 1, 1);
    clear_floor(&mut run);
    assert_eq!(run.status, Status::Victory);
    assert_eq!(run.floor, 1);
    assert_eq!(run.descent_words(content().unwrap()), Vec::<String>::new());
    let vote = RunEvent::FloorVoted {
        hero: 0,
        descend: true,
    };
    assert!(run.apply(content().unwrap(), &vote).is_err());
}

#[rstest]
#[case::healthy(3, 100, true)]
#[case::two_hearts_at_sixty(2, 60, true)]
#[case::one_heart(1, 100, false)]
#[case::hurt(3, 55, false)]
fn bots_descend_with_two_hearts_and_most_of_their_life(
    #[case] hearts: u8,
    #[case] percent: i32,
    #[case] descend: bool,
) {
    let content = content().unwrap();
    let mut run = started(2, 1, CONTENT, 1, 2);
    clear_floor(&mut run);
    run.hearts = hearts;
    for h in &mut run.heroes {
        h.life = h.max_life(content) * percent / 100;
    }
    assert_eq!(
        run.bot_moves(content),
        Vec::new(),
        "bots wait for the player"
    );
    vote(&mut run, 0, descend);
    let moves = run.bot_moves(content);
    assert_eq!(
        moves,
        vec![
            RunEvent::FloorVoted { hero: 1, descend },
            RunEvent::FloorVoted { hero: 2, descend },
        ]
    );
    assert_ne!(run.descent_words(content), Vec::<String>::new());
}
