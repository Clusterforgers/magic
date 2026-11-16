//! Goal: the rank × Heat table never makes a boss easier: at every party size a later boss on the ladder and a higher
//! Heat give it at least as much life, hand, brain and minions, its relic and modifier no later and its Enrage no
//! later; the first boss at Heat 0 is gentler than the boss as written and the final boss at Heat 1 is not; each
//! Enrage has a variant for each Heat's turn; the run lists each boss of its ladder with the life of its rank.
use mtg_campaign::content::{Boss, CardKind};
use mtg_campaign::scale::{ENRAGE_TURN, ENRAGE_WRITTEN, MAX_HEAT, RANKS, enrage_id};
use mtg_campaign::{Run, RunEvent, Scale, content};
use rstest::rstest;

#[rstest]
fn the_ladder_lists_each_boss_with_the_life_of_its_rank(
    #[values(1, 3, 6)] heroes: usize,
    #[values(0, 2)] heat: u8,
) {
    let content = content().unwrap();
    let theme = &content.themes[0];
    let events: Vec<RunEvent> = (0..heroes)
        .map(|at| RunEvent::Joined {
            name: format!("H{at}"),
            user: Some(format!("u{at}")),
            bot: None,
        })
        .collect();
    let run = Run::replay(content, "warrens", 9, heat, &events).unwrap();
    let lives = run.boss_lives(content);
    assert_eq!(lives.len(), theme.bosses.len());
    for node in run.map.nodes.iter().filter(|n| n.boss.is_some()) {
        let boss = theme.boss(node.boss.as_deref().unwrap()).unwrap();
        let life = Scale::new(heroes, heat, run.boss_rank(node.id)).life(boss);
        assert!(lives.contains(&(boss.id.clone(), life)), "{}", boss.id);
    }
}

/// Everything that makes the boss harder, in an order where more is harder (the Enrage turn negated).
fn strength(scale: Scale, boss: &Boss) -> [i64; 7] {
    let tier = scale.tier();
    [
        i64::from(scale.life(boss)),
        i64::from(scale.hand(boss)),
        scale.brain(boss.brain) as i64,
        i64::try_from(scale.minion_count()).unwrap(),
        i64::from(tier.relic),
        i64::from(tier.modifier),
        -i64::from(scale.enrage_turn()),
    ]
}

fn no_easier(harder: Scale, easier: Scale, boss: &Boss) {
    let (a, b) = (strength(harder, boss), strength(easier, boss));
    assert!(
        a.iter().zip(&b).all(|(a, b)| a >= b),
        "{harder:?} {a:?} is easier than {easier:?} {b:?} for {}",
        boss.id
    );
}

#[rstest]
fn a_later_boss_and_a_higher_heat_are_never_easier(
    #[values(1, 2, 3, 4, 5, 6)] heroes: usize,
    #[values(0, 1, 2)] heat: u8,
) {
    for boss in &content().unwrap().themes[0].bosses {
        for rank in 0..=RANKS {
            let here = Scale::new(heroes, heat, rank);
            no_easier(Scale::new(heroes, heat, rank + 1), here, boss);
            if heat < MAX_HEAT {
                no_easier(Scale::new(heroes, heat + 1, rank), here, boss);
            }
        }
    }
}

#[rstest]
fn the_ladder_ramps_from_a_gentle_first_boss(#[values(1, 2, 3, 4, 5, 6)] heroes: usize) {
    for boss in &content().unwrap().themes[0].bosses {
        let first = Scale::new(heroes, 0, 0);
        let written = Scale::new(heroes, 1, 2);
        let last = Scale::new(heroes, 1, RANKS - 1);
        // The first boss at Heat 0: less life, a smaller hand and a weaker brain than as written, no relic.
        assert!(first.life(boss) < written.life(boss));
        assert!(first.hand(boss) < written.hand(boss));
        assert!(first.brain(boss.brain) < boss.brain);
        assert!(!first.tier().relic);
        // The final boss at Heat 1 is at least the boss as written; Heat 2 plays it smarter.
        assert!(last.life(boss) >= written.life(boss));
        assert_eq!(written.brain(boss.brain), boss.brain);
        assert!(Scale::new(heroes, 2, 1).brain(boss.brain) > boss.brain);
        // Each step of the ladder adds life.
        for rank in 1..RANKS {
            for heat in 0..=MAX_HEAT {
                assert!(
                    Scale::new(heroes, heat, rank).life(boss)
                        > Scale::new(heroes, heat, rank - 1).life(boss)
                );
            }
        }
    }
}

#[test]
fn heat_1_plays_the_enrage_as_written_and_the_others_their_variant() {
    let content = content().unwrap();
    assert_eq!(ENRAGE_TURN[1], ENRAGE_WRITTEN);
    let enrages: Vec<_> = content
        .cards
        .values()
        .filter(|c| c.kind == CardKind::Enrage && c.turn == ENRAGE_WRITTEN)
        .collect();
    assert_eq!(enrages.len(), content.themes[0].bosses.len());
    for card in enrages {
        for turn in ENRAGE_TURN {
            let variant = content.card(&enrage_id(&card.id, turn)).unwrap();
            assert_eq!(variant.turn, turn);
            assert_eq!((&variant.name, variant.kind), (&card.name, card.kind));
            let abilities = serde_json::to_string(&variant.abilities).unwrap();
            assert!(
                abilities.contains(&format!("\"right\":{turn}")),
                "{}: {abilities}",
                variant.id
            );
            let beyond = format!("{{\"TurnsTaken\":\"Opponents\"}},{}]", turn - 1);
            let counts = serde_json::to_string(&card.abilities)
                .unwrap()
                .contains("\"Difference\"");
            assert_eq!(
                abilities.contains(&beyond),
                counts,
                "{}: {abilities}",
                variant.id
            );
            if turn != ENRAGE_WRITTEN {
                assert_ne!(variant.text, card.text, "{} words its turn", variant.id);
            }
        }
    }
}
