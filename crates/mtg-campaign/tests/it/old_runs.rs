//! Goal: a stored run of a generation before themed floors replays its events to the same run as when it was played,
//! descents included (the fixture holds the events and the run they folded to; `MTG_UPDATE_OLD_RUNS=1` rewrites it).
use mtg_campaign::{Brain, Run, RunEvent, Status, content};
use rstest::rstest;
use serde::{Deserialize, Serialize};

use crate::endless::deck;

/// One played run: how it was made, its events and what they folded to.
#[derive(Serialize, Deserialize)]
struct Played {
    theme: String,
    seed: u64,
    heat: u8,
    events: Vec<RunEvent>,
    run: serde_json::Value,
}

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/it/old_runs.json");

/// Plays a run of `version` through `floors` floors, every hero answering like a bot, and banks the win.
fn play(theme: &str, seed: u64, version: u32, floors: u8) -> Played {
    let content = content().unwrap();
    let mut events = vec![RunEvent::Content { version }];
    for at in 0..2u8 {
        events.push(RunEvent::Joined {
            name: format!("H{at}"),
            user: (at == 0).then(|| "u0".to_owned()),
            bot: (at > 0).then_some(Brain::Normal),
        });
        events.push(RunEvent::DeckChosen {
            hero: at,
            deck: deck(),
        });
    }
    events.push(RunEvent::Started);
    let mut run = Run::replay(content, theme, seed, 0, &events).unwrap();
    for _ in 0..2000 {
        let next: Vec<RunEvent> = match &run.status {
            Status::Fight { game: None } => vec![RunEvent::FightCreated {
                game: format!("g{}", run.fights.len()),
            }],
            Status::Fight { game: Some(_) } => vec![RunEvent::FightEnded {
                won: true,
                lives: run.heroes.iter().map(|h| h.life).collect(),
                spirits: Vec::new(),
            }],
            Status::Map | Status::Room => {
                let mut as_bots = run.clone();
                for h in &mut as_bots.heroes {
                    h.bot = Some(Brain::Normal);
                    h.user = None;
                }
                as_bots.bot_moves(content)
            }
            Status::FloorCleared => (0..2u8)
                .map(|hero| RunEvent::FloorVoted {
                    hero,
                    descend: run.floor < floors,
                })
                .collect(),
            _ => break,
        };
        for event in next {
            run.apply(content, &event).unwrap();
            events.push(event);
        }
    }
    assert_eq!(run.status, Status::Victory);
    Played {
        theme: theme.to_owned(),
        seed,
        heat: 0,
        events,
        run: serde_json::to_value(&run).unwrap(),
    }
}

fn runs() -> Vec<Played> {
    vec![
        play("warrens", 11, 8, 3),
        play("spire", 12, 8, 3),
        play("warrens", 13, 6, 2),
    ]
}

#[test]
fn old_runs_replay_unchanged() {
    if std::env::var_os("MTG_UPDATE_OLD_RUNS").is_some() {
        std::fs::write(FIXTURE, serde_json::to_string(&runs()).unwrap()).unwrap();
    }
    let stored: Vec<Played> =
        serde_json::from_str(&std::fs::read_to_string(FIXTURE).unwrap()).unwrap();
    assert_eq!(stored.len(), 3);
    for played in stored {
        let run = Run::replay(
            content().unwrap(),
            &played.theme,
            played.seed,
            played.heat,
            &played.events,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&run).unwrap(),
            played.run,
            "{}",
            played.theme
        );
        assert!(run.floor >= 2, "the run descended");
    }
}

#[rstest]
fn an_old_run_keeps_its_theme_on_every_floor(#[values(6, 7, 8)] version: u32) {
    let played = play("warrens", 21, version, 3);
    let run = Run::replay(content().unwrap(), "warrens", 21, 0, &played.events).unwrap();
    assert_eq!(run.floor, 3);
    assert_eq!(run.floor_theme(), "warrens");
    let warrens = content().unwrap().theme("warrens").unwrap();
    for fight in &run.fights {
        let boss = fight.boss.as_deref().unwrap();
        assert!(warrens.boss(boss).is_some(), "{boss} is a Warrens boss");
    }
}
