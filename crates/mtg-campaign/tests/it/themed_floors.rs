//! Goal: from the themed generation each descent leads into another open theme drawn by the seed and the floor, so a
//! long run passes through several and never plays one theme on two floors in a row; the floor's bosses, events and
//! reward pool are its theme's, and the chronicle and the descent's summary name the new theme.
use mtg_campaign::run::{CONTENT, THEMES_FROM, in_sentence};
use mtg_campaign::{Offer, RoomKind, Run, content};
use proptest::prelude::*;
use rstest::rstest;

use crate::endless::{clear_floor, started, vote};

/// Clears floors and descends until the party stands on `floors`, checking each floor against its theme.
fn descend_to(run: &mut Run, floors: u8) -> Vec<String> {
    let content = content().unwrap();
    let mut visited = vec![run.floor_theme().to_owned()];
    while run.floor < floors {
        clear_floor(run);
        let words = run.descent_words(content);
        let next = run.next_floor_theme(content).unwrap().unwrap();
        assert!(
            words.iter().any(|w| w.contains(&in_sentence(&next.name))),
            "{words:?} names {}",
            next.name
        );
        for hero in 0..run.heroes.len() {
            vote(run, hero, true);
        }
        clear_offers(run);
        assert_eq!(run.floor_theme(), next.id);
        let theme = content.theme(run.floor_theme()).unwrap();
        for boss in run.map.nodes.iter().filter_map(|n| n.boss.as_deref()) {
            assert!(theme.boss(boss).is_some(), "{boss} is of {}", theme.id);
        }
        assert_eq!(
            run.theme_from.as_deref(),
            visited.last().map(String::as_str)
        );
        assert!(
            run.log
                .iter()
                .any(|l| l.ends_with(&format!("descends into {}.", in_sentence(&theme.name)))),
            "the chronicle names the theme"
        );
        visited.push(run.floor_theme().to_owned());
    }
    visited
}

/// Answers the descent loot like a bot until the party is on the next floor.
fn clear_offers(run: &mut Run) {
    let content = content().unwrap();
    for _ in 0..50 {
        if run.heroes.iter().all(|h| h.offers.is_empty()) {
            return;
        }
        let mut as_bots = run.clone();
        for h in &mut as_bots.heroes {
            h.bot = Some(mtg_campaign::Brain::Normal);
            h.user = None;
        }
        for event in as_bots.bot_moves(content) {
            run.apply(content, &event).unwrap();
        }
    }
}

#[rstest]
fn a_long_run_passes_through_several_themes(#[values(1, 2, 3, 4, 5, 6)] seed: u64) {
    let mut run = started(seed, 1, CONTENT, 1, 1);
    let visited = descend_to(&mut run, 6);
    assert_eq!(visited.len(), 6);
    for pair in visited.windows(2) {
        assert_ne!(pair[0], pair[1], "no theme twice in a row: {visited:?}");
    }
    let distinct: std::collections::BTreeSet<&String> = visited.iter().collect();
    assert!(distinct.len() >= 2, "{visited:?}");
}

#[rstest]
fn a_new_floors_camps_follow_its_theme(#[values(7, 8)] seed: u64) {
    let content = content().unwrap();
    let mut run = started(seed, 1, CONTENT, 1, 1);
    descend_to(&mut run, 2);
    let theme = content.theme(run.floor_theme()).unwrap().clone();
    clear_until_room(&mut run);
    for hero in &run.heroes {
        for offer in &hero.offers {
            match offer {
                Offer::Event { event } => assert!(theme.event(event).is_some(), "{event}"),
                Offer::Cards { cards } => {
                    for card in cards {
                        assert!(theme.pool.iter().any(|(name, _)| name == card), "{card}");
                    }
                }
                _ => {}
            }
        }
    }
}

/// Plays the floor until the party stands in a camp.
fn clear_until_room(run: &mut Run) {
    let content = content().unwrap();
    for _ in 0..40 {
        if run
            .at
            .and_then(|at| run.map.node(at))
            .is_some_and(|n| n.kind != RoomKind::Boss)
        {
            return;
        }
        let event = match &run.status {
            mtg_campaign::Status::Fight { game: None } => mtg_campaign::RunEvent::FightCreated {
                game: format!("g{}", run.fights.len()),
            },
            mtg_campaign::Status::Fight { game: Some(_) } => mtg_campaign::RunEvent::FightEnded {
                won: true,
                lives: run.heroes.iter().map(|h| h.life).collect(),
                spirits: Vec::new(),
            },
            _ => {
                let mut as_bots = run.clone();
                for h in &mut as_bots.heroes {
                    h.bot = Some(mtg_campaign::Brain::Normal);
                    h.user = None;
                }
                for event in as_bots.bot_moves(content) {
                    run.apply(content, &event).unwrap();
                }
                continue;
            }
        };
        run.apply(content, &event).unwrap();
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn the_next_theme_is_another_open_one(seed in any::<u64>(), floor in 2u8..40) {
        let content = content().unwrap();
        let open = content.open_themes(THEMES_FROM);
        for from in &open {
            let next = content.next_theme(seed, THEMES_FROM, floor, &from.id).unwrap();
            prop_assert_eq!(next, content.next_theme(seed, THEMES_FROM, floor, &from.id).unwrap());
            prop_assert!(open.contains(&next));
            prop_assert!(open.len() < 2 || next.id != from.id);
        }
    }
}

#[test]
fn a_lone_open_theme_stays() {
    let content = content().unwrap();
    let warrens = content.next_theme(3, 4, 2, "warrens").unwrap();
    assert_eq!(warrens.id, "warrens");
}
