//! Goal: endless Delves holds together: a cleared floor of the endless generation waits for the party's vote to bank
//! the win or descend (most votes, a tie to the torch bearer; bots descend only with two hearts and 60 % life); a
//! deeper floor's ladder is drawn by seed and floor and avoids the bosses just fought; its bosses play the Heat's top
//! tier at a bracket one higher per floor, capped at 5 with life, hand and brain beyond the cap; they bring a risky
//! relic more per floor and are Intensified (an earlier Enrage, the passive once more); the first floor plays as
//! generation 4 and runs of older generations end in victory as before.
use mtg_campaign::content::Boss;
use mtg_campaign::map::{LIEUTENANTS, generate_floor, generate_for};
use mtg_campaign::run::{CONTENT, ENDLESS_FROM, HEARTS};
use mtg_campaign::scale::{
    BRACKET_TIERS, BRACKETS, BRACKETS_FROM, ENRAGE_EARLIEST, ENRAGE_TURN, MAX_BRACKET, MAX_HEAT,
    MOST, OVERFLOW_LIFE, PAIRS_FROM, RANKS, UBER_HEAT, enrage_id,
};
use mtg_campaign::{Brain, HeroDeck, Run, RunEvent, Scale, Status, content};
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

/// The plan of the fight against the boss at `rank` of the run's floor.
fn plan_at(run: &Run, rank: usize) -> mtg_campaign::FightPlan {
    let node = run
        .map
        .nodes
        .iter()
        .filter(|n| n.boss.is_some())
        .nth(rank)
        .unwrap()
        .id;
    let mut at = run.clone();
    at.at = Some(node);
    at.status = Status::Fight { game: None };
    at.fight_plan(content().unwrap()).unwrap().unwrap()
}

fn boss(id: &str) -> &'static Boss {
    content().unwrap().themes[0].boss(id).unwrap()
}

fn risky(id: &str) -> bool {
    content()
        .unwrap()
        .cards
        .get(id)
        .is_some_and(|c| c.role == "Risky")
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
    let mut run = started(5, 1, PAIRS_FROM, players, 0);
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
    let mut run = started(9, 1, PAIRS_FROM, 1, 1);
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
    assert_eq!(run.map, generate_floor(theme, 9, PAIRS_FROM, 1, 2, &avoid));
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
    fn a_deeper_floor_is_drawn_by_seed_and_avoids_the_bosses_just_fought(seed in any::<u64>(), heat in 0..=MAX_HEAT, floor in 2u8..6) {
        let theme = &content().unwrap().themes[0];
        let above = generate_floor(theme, seed, CONTENT, heat, floor - 1, &[]);
        let fought: Vec<&str> = above.nodes.iter().filter_map(|n| n.boss.as_deref()).collect();
        let map = generate_floor(theme, seed, CONTENT, heat, floor, &fought);
        prop_assert_eq!(&map, &generate_floor(theme, seed, CONTENT, heat, floor, &fought));
        let ids: Vec<&str> = map.nodes.iter().filter_map(|n| n.boss.as_deref()).collect();
        prop_assert_eq!(ids.len(), LIEUTENANTS + 1);
        let last = theme.boss(ids[LIEUTENANTS]).unwrap();
        prop_assert!(last.final_boss || last.uber);
        prop_assert!(ids[..LIEUTENANTS].iter().all(|id| !theme.boss(id).unwrap().uber));
        let identity = |id: &str| theme.boss(id).unwrap().identity().to_owned();
        let met: Vec<String> = fought.iter().map(|id| identity(id)).collect();
        let fresh = theme.bosses.iter().filter(|b| !b.final_boss && !b.uber && b.since <= CONTENT && !met.contains(&b.id)).count();
        let repeats = ids[..LIEUTENANTS].iter().filter(|id| met.contains(&identity(id))).count();
        prop_assert_eq!(repeats, LIEUTENANTS.saturating_sub(fresh));
    }
}

#[test]
fn a_deeper_floor_takes_the_roster_of_the_generation() {
    let theme = &content().unwrap().themes[0];
    let uber = |id: &str| theme.boss(id).unwrap().uber;
    let last = |map: &mtg_campaign::map::FloorMap| map.last().and_then(|n| n.boss.clone()).unwrap();
    let mut met = 0;
    let mut floors = 0;
    for seed in 0..200u64 {
        let mut above: Vec<String> = Vec::new();
        for floor in 1..=5u8 {
            let avoid: Vec<&str> = above.iter().map(String::as_str).collect();
            let older = generate_floor(theme, seed, BRACKETS_FROM, 2, floor, &avoid);
            for id in older.nodes.iter().filter_map(|n| n.boss.as_deref()) {
                assert!(theme.boss(id).unwrap().since <= BRACKETS_FROM, "{id}");
            }
            for heat in 0..UBER_HEAT {
                let cool = generate_floor(theme, seed, CONTENT, heat, floor, &avoid);
                assert!(
                    cool.nodes
                        .iter()
                        .filter_map(|n| n.boss.as_deref())
                        .all(|id| !uber(id)),
                    "no uber boss below its Heat"
                );
            }
            let cool = generate_floor(theme, seed, CONTENT, 1, floor, &avoid);
            let hot = generate_floor(theme, seed, CONTENT, UBER_HEAT, floor, &avoid);
            let ids: Vec<&str> = hot.nodes.iter().filter_map(|n| n.boss.as_deref()).collect();
            assert!(
                ids[..LIEUTENANTS].iter().all(|id| !uber(id)),
                "never a lieutenant"
            );
            floors += 1;
            if uber(&last(&hot)) {
                met += 1;
                assert!(
                    !above.iter().any(|id| uber(id)),
                    "not on two floors in a row"
                );
                assert_eq!(
                    hot.nodes[..hot.nodes.len() - 1],
                    cool.nodes[..cool.nodes.len() - 1],
                    "only the final boss differs"
                );
            } else {
                assert_eq!(hot, cool);
            }
            above = hot.nodes.iter().filter_map(|n| n.boss.clone()).collect();
        }
    }
    assert!(
        met > 0 && met * 100 / floors < 40,
        "{met} of {floors} floors"
    );
}

#[test]
fn floors_differ_by_seed_and_floor() {
    let theme = &content().unwrap().themes[0];
    let ladder = |seed: u64, floor: u8| {
        let map = generate_floor(theme, seed, CONTENT, 1, floor, &[]);
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
fn a_deeper_floor_has_no_early_handicap(
    #[values(0, 1, 2)] heat: u8,
    #[values(1, 2, 4, 6)] heroes: usize,
    #[values(2, 3, 5)] floor: u8,
) {
    let top = BRACKET_TIERS[usize::from(heat)][RANKS - 1];
    for rank in 0..RANKS {
        let tier = Scale::of_content(heroes, heat, rank, CONTENT)
            .on_floor(floor)
            .tier();
        assert!(tier.relic && tier.modifier, "rank {rank}");
        assert!(
            tier.life >= top.life && tier.hand >= top.hand,
            "rank {rank}"
        );
        assert!(
            tier.brain >= top.brain && tier.minions >= top.minions,
            "rank {rank}"
        );
    }
    let run = {
        let mut run = started(4, heat, CONTENT, heroes.min(2), heroes.saturating_sub(2));
        run.floor = floor;
        run
    };
    let plan = plan_at(&run, 0);
    let own = &boss(&plan.boss.boss).relic;
    assert!(
        own.is_empty() || plan.boss.rules.contains(own),
        "the first boss brings its relic"
    );
}

#[rstest]
fn the_bracket_table_shifts_by_floor_and_is_capped(
    #[values(0, 1, 2)] heat: u8,
    #[values(1, 2, 3, 4, 6)] floor: u8,
) {
    for rank in 0..RANKS {
        let base = Scale::of_content(2, heat, rank, CONTENT);
        let scale = base.on_floor(floor);
        let wanted = BRACKETS[usize::from(heat)][rank] + floor - 1;
        assert_eq!(scale.bracket(), wanted.min(MAX_BRACKET));
        let over = wanted.saturating_sub(MAX_BRACKET);
        assert_eq!(scale.overflow(), over);
        if floor == 1 {
            assert_eq!(scale.tier(), base.tier());
            continue;
        }
        let top = BRACKET_TIERS[usize::from(heat)][RANKS - 1];
        let tier = scale.tier();
        assert_eq!(tier.life, top.life + OVERFLOW_LIFE * i32::from(over));
        assert_eq!(tier.hand, top.hand + i32::from(over));
        assert_eq!(tier.brain, top.brain + i8::try_from(over).unwrap());
        let shallower = base.on_floor(floor - 1);
        assert!(
            scale.life(&content().unwrap().themes[0].bosses[0])
                >= shallower.life(&content().unwrap().themes[0].bosses[0])
        );
        assert!(scale.brain(Brain::Normal) >= shallower.brain(Brain::Normal));
    }
}

#[rstest]
fn deeper_bosses_bring_a_risky_relic_more_per_floor(#[values(2, 3, 4)] floor: u8) {
    let mut run = started(11, 1, CONTENT, 1, 1);
    run.heroes[0].relics.push(
        content()
            .unwrap()
            .relics()
            .into_iter()
            .find(|c| c.role == "Risky")
            .unwrap()
            .id
            .clone(),
    );
    let shallow = run.clone();
    run.floor = floor;
    for rank in 0..RANKS {
        let plan = plan_at(&run, rank);
        let own = &boss(&plan.boss.boss).relic;
        let extra: Vec<&String> = plan
            .boss
            .rules
            .iter()
            .filter(|id| risky(id) && *id != own)
            .collect();
        assert_eq!(extra.len(), usize::from(floor - 1), "rank {rank}");
        let mut distinct = extra.clone();
        distinct.dedup();
        assert_eq!(distinct.len(), extra.len());
        assert!(
            extra.iter().all(|id| !run.heroes[0].relics.contains(id)),
            "never a hero's relic"
        );
        assert_eq!(plan, plan_at(&run, rank), "drawn by seed");
        let first = plan_at(&shallow, rank);
        assert!(
            !first.boss.rules.iter().any(|id| risky(id) && id != own),
            "none on floor 1"
        );
    }
}

#[rstest]
fn deeper_bosses_are_intensified(#[values(0, 1, 2)] heat: u8, #[values(2, 3, 4, 8)] floor: u8) {
    let content = content().unwrap();
    let deeper = u32::from(floor - 1);
    for boss in content.themes[0]
        .bosses
        .iter()
        .chain(&content.themes[0].upgraded)
    {
        let scale = Scale::of_content(3, heat, 1, CONTENT).on_floor(floor);
        let turn = ENRAGE_TURN[usize::from(heat)]
            .saturating_sub(deeper)
            .max(ENRAGE_EARLIEST);
        assert_eq!(scale.enrage_turn(), turn);
        let rules = scale.rules(boss);
        if !boss.enrage.is_empty() {
            let id = enrage_id(&boss.enrage, turn);
            assert!(
                rules.contains(&id) && content.cards.contains_key(&id),
                "{id}"
            );
        }
        for passive in &boss.rules {
            let times = rules.iter().filter(|id| *id == passive).count();
            let written = boss.rules.iter().filter(|id| *id == passive).count();
            assert_eq!(times, written * usize::from(floor), "{passive}");
        }
        assert_ne!(scale.intensify(), Vec::<String>::new());
    }
    assert_eq!(
        Scale::of_content(3, heat, 1, CONTENT).intensify(),
        Vec::<String>::new()
    );
}

#[test]
fn the_first_floor_plays_as_generation_four() {
    let theme = &content().unwrap().themes[0];
    for heat in 0..=MAX_HEAT {
        for heroes in 1..=MOST {
            for rank in 0..=RANKS {
                assert_eq!(
                    Scale::of_content(heroes, heat, rank, CONTENT),
                    Scale::of_content(heroes, heat, rank, BRACKETS_FROM)
                );
            }
        }
    }
    for seed in 0..16 {
        assert_eq!(
            generate_floor(theme, seed, CONTENT, 1, 1, &[]),
            generate_for(theme, seed, CONTENT, 1)
        );
        for heat in 0..=MAX_HEAT {
            assert_eq!(
                generate_floor(theme, seed, CONTENT, heat, 1, &[]),
                generate_for(theme, seed, CONTENT, heat)
            );
        }
        let old = started(seed, 1, BRACKETS_FROM, 2, 0);
        let mut new = started(seed, 1, CONTENT, 2, 0);
        new.map = old.map.clone();
        for rank in 0..RANKS {
            assert_eq!(
                plan_at(&new, rank),
                plan_at(&old, rank),
                "seed {seed} rank {rank}"
            );
        }
    }
}

#[rstest]
fn older_runs_end_in_victory_as_before(#[values(0, 3, 4)] version: u32) {
    assert!(version < ENDLESS_FROM);
    let mut run = started(7, 1, version, 1, 1);
    clear_floor(&mut run);
    assert_eq!(run.status, Status::Victory);
    assert_eq!(run.floor, 1);
    assert_eq!(run.descent_words(), Vec::<String>::new());
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
    assert_ne!(run.descent_words(), Vec::<String>::new());
}
