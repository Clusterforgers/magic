//! Goal: from the descent-loot generation a descent first gives every hero the final boss's loot (gold, a relic pick
//! and a card pick, as after a lieutenant) and enters the next floor's first fight once every hero answered, for every
//! party size and Heat; banking still ends the run at once; runs of an older endless generation descend straight to
//! the next fight as before; and the run's depth counts the floors cleared, then the bosses beaten below them.
use crate::endless::{clear_floor, started, vote};
use mtg_campaign::map::generate_floor;
use mtg_campaign::run::{CONTENT, DESCENT_LOOT_FROM, ENDLESS_FROM, HEARTS, LOOT_GOLD};
use mtg_campaign::scale::PAIRS_FROM;
use mtg_campaign::{Brain, Offer, Run, RunEvent, Status, content};
use rstest::rstest;

/// Answers every offer as a bot would until the party leaves the room.
fn answer_offers(run: &mut Run) {
    let content = content().unwrap();
    for _ in 0..64 {
        if run.status != Status::Room {
            return;
        }
        let mut as_bots = run.clone();
        for h in &mut as_bots.heroes {
            h.bot = Some(Brain::Normal);
            h.user = None;
        }
        for event in as_bots.bot_moves(content) {
            run.apply(content, &event).unwrap();
        }
    }
    panic!("the offers never ended");
}

fn vote_all(run: &mut Run, descend: bool) {
    for hero in 0..run.heroes.len() {
        if run.status == Status::FloorCleared {
            vote(run, hero, descend);
        }
    }
}

fn bosses(run: &Run) -> Vec<String> {
    run.map
        .nodes
        .iter()
        .filter_map(|n| n.boss.clone())
        .collect()
}

#[rstest]
fn a_descent_gives_the_final_boss_loot_first(
    #[values((1, 0), (2, 0), (1, 2), (3, 1), (6, 0))] party: (usize, usize),
    #[values(0, 1, 2)] heat: u8,
) {
    let (players, bots) = party;
    let content = content().unwrap();
    let mut run = started(17 + u64::from(heat), heat, CONTENT, players, bots);
    clear_floor(&mut run);
    assert_eq!(run.status, Status::FloorCleared);
    run.hearts = 1;
    let before = run.clone();
    let fought = bosses(&run);
    vote_all(&mut run, true);
    assert_eq!(
        run.status,
        Status::Room,
        "the loot waits before the descent"
    );
    assert!(run.descending);
    assert_eq!(
        (run.floor, run.at),
        (1, before.at),
        "still on the cleared floor"
    );
    for (h, was) in run.heroes.iter().zip(&before.heroes) {
        assert_eq!(h.gold, was.gold + LOOT_GOLD, "{}", h.name);
        assert!(
            matches!(h.offers.as_slice(), [Offer::Relics { relics }, Offer::Cards { .. }] if !relics.is_empty()),
            "{}: {:?}",
            h.name,
            h.offers
        );
    }
    assert!(
        run.apply(
            content,
            &RunEvent::FloorVoted {
                hero: 0,
                descend: true
            }
        )
        .is_err(),
        "the vote is over"
    );
    answer_offers(&mut run);
    assert!(!run.descending);
    assert_eq!(run.floor, 2);
    assert_eq!(
        run.status,
        Status::Fight { game: None },
        "straight into the next fight"
    );
    assert_eq!(run.hearts, 2, "one heart back");
    let avoid: Vec<&str> = fought.iter().map(String::as_str).collect();
    let theme = &content.themes[0];
    assert_eq!(
        run.map,
        generate_floor(theme, run.seed, CONTENT, heat, 2, &avoid)
    );
    assert_eq!(run.at, run.map.choices(None).first().copied());
    for (h, was) in run.heroes.iter().zip(&before.heroes) {
        assert_eq!(
            h.relics.len(),
            was.relics.len() + 1,
            "{} took a relic",
            h.name
        );
        assert_eq!(h.life, was.life, "{}: no heal beyond the fight's", h.name);
    }
    // The run replays to the same state from its events.
    let mut again = started(17 + u64::from(heat), heat, CONTENT, players, bots);
    clear_floor(&mut again);
    again.hearts = 1;
    vote_all(&mut again, true);
    assert_eq!(again.heroes, {
        let mut r = before.clone();
        vote_all(&mut r, true);
        r.heroes
    });
}

#[rstest]
fn banking_takes_no_loot(#[values(1, 3)] players: usize, #[values(0, 2)] heat: u8) {
    let mut run = started(23, heat, CONTENT, players, 0);
    clear_floor(&mut run);
    let before = run.clone();
    vote_all(&mut run, false);
    assert_eq!(run.status, Status::Victory);
    assert!(!run.descending);
    assert_eq!(run.heroes, before.heroes, "no gold, no offers");
}

#[rstest]
fn older_endless_runs_descend_straight_to_the_fight(
    #[values(ENDLESS_FROM, PAIRS_FROM)] version: u32,
    #[values(1, 2)] players: usize,
) {
    assert!(version < DESCENT_LOOT_FROM);
    let mut run = started(29, 1, version, players, 0);
    clear_floor(&mut run);
    let before = run.clone();
    assert!(
        !run.descent_words().iter().any(|w| w.contains("loot")),
        "no loot promised"
    );
    vote_all(&mut run, true);
    assert_eq!(run.floor, 2);
    assert_eq!(run.status, Status::Fight { game: None });
    assert_eq!(run.heroes, before.heroes, "no loot");
}

#[test]
fn the_descent_choice_names_the_loot() {
    let mut run = started(31, 1, CONTENT, 2, 0);
    clear_floor(&mut run);
    let words = run.descent_words();
    assert!(words[0].contains("final boss's loot"), "{words:?}");
}

#[test]
fn depth_counts_floors_then_bosses_below() {
    let content = content().unwrap();
    let mut run = started(37, 1, CONTENT, 1, 0);
    assert_eq!(run.depth(), (0, 0));
    run.apply(content, &RunEvent::FightCreated { game: "g0".into() })
        .unwrap();
    let won = |run: &Run| RunEvent::FightEnded {
        won: true,
        lives: run.heroes.iter().map(|h| h.life).collect(),
        spirits: Vec::new(),
    };
    run.apply(content, &won(&run)).unwrap();
    assert_eq!(run.depth(), (0, 1), "one boss of the first floor");
    clear_floor(&mut run);
    assert_eq!(run.depth(), (1, 0), "a cleared floor");
    run.hearts = HEARTS;
    vote_all(&mut run, true);
    assert_eq!(run.depth(), (1, 0), "still while the loot waits");
    answer_offers(&mut run);
    run.apply(content, &RunEvent::FightCreated { game: "g9".into() })
        .unwrap();
    run.apply(content, &won(&run)).unwrap();
    assert_eq!(run.depth(), (1, 1), "a boss of the second floor");
}
