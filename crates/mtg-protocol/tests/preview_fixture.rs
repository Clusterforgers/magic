//! Goal: the block-preview fixture the web tests replay is exactly what the engine answers a seat's `Preview` messages with (blocks, attacks, a refused answer), and every reply in it is the shape the wire carries.
//!
//! Regenerate with `MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol --test preview_fixture`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Engine};
use mtg_protocol::{ClientMessage, DecisionId, ServerMessage};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, damaged, ready};
use serde::Deserialize;

const FIXTURE: &str = "web/src/protocol/fixtures/block-preview.json";
const DESCRIPTION: &str = "What mtg-server answers a seat's Preview messages with, from the real engine: \
    `blocks` is player 1 facing three attackers (a Decision, then Preview and Forecast pairs), `attacks` is player 0 \
    declaring them. A reply with a null forecast is an answer the engine would refuse or a declaration of no attackers. \
    Regenerate: MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol --test preview_fixture";
/// Player 1's life: low enough that some attacks and no-blocks are lethal.
const LIFE: i32 = 6;

#[derive(Debug, Deserialize)]
struct Exchange {
    label: String,
    preview: ClientMessage,
    reply: ServerMessage,
}

#[derive(Debug, Deserialize)]
struct Scene {
    seat: PlayerId,
    decision: ServerMessage,
    exchanges: Vec<Exchange>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    description: String,
    blocks: Scene,
    attacks: Scene,
}

fn json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap()
}

/// A scene is one pending decision and a labelled answer to preview for each exchange.
struct Script {
    seat: PlayerId,
    id: DecisionId,
    answers: Vec<(&'static str, Answer)>,
}

/// The scene as the committed file shows it: the decision, then one line per exchange.
///
/// Each reply is what the server does with a `Preview` for the decision it just sent: one `Forecast`,
/// empty when the engine has none.
fn render_scene(engine: &Engine, script: Script) -> String {
    let decision = engine.decision().expect("a pending decision");
    let opening = ServerMessage::Decision {
        id: script.id,
        decision: decision.clone(),
    };
    let lines: Vec<String> = script
        .answers
        .into_iter()
        .map(|(label, answer)| {
            let reply = ServerMessage::Forecast {
                decision: script.id,
                forecast: engine.preview_forecast(script.seat, &answer).ok(),
            };
            let preview = ClientMessage::Preview {
                decision: script.id,
                answer,
            };
            format!(
                "      {{\"label\": {}, \"preview\": {}, \"reply\": {}}}",
                json(&label),
                json(&preview),
                json(&reply)
            )
        })
        .collect();
    format!(
        "{{\n    \"seat\": {},\n    \"decision\": {},\n    \"exchanges\": [\n{}\n    ]\n  }}",
        json(&script.seat),
        json(&opening),
        lines.join(",\n")
    )
}

/// Three attackers against two blockers and `LIFE` life.
fn scenario() -> Scenario {
    Scenario::new(2)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(free_creature("Ember Wolf", 4, 4)),
                ready(free_creature("Cave Imp", 2, 2)),
                ready(free_creature("Glass Cannon", 5, 1)),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Stone Guard", 2, 2)),
                damaged(free_creature("Marsh Bear", 3, 3), 1),
            ],
        )
        .life(1, LIFE)
}

fn attack(driver: &Driver, names: &[&str]) -> Answer {
    let them = AttackTarget::Player(PlayerId(1));
    Answer::DeclareAttackers(
        names
            .iter()
            .map(|name| (driver.id_of(name).unwrap(), them))
            .collect(),
    )
}

fn block(driver: &Driver, pairs: &[(&str, &str)]) -> Answer {
    Answer::DeclareBlockers(
        pairs
            .iter()
            .map(|(blocker, attacker)| {
                (
                    driver.id_of(blocker).unwrap(),
                    driver.id_of(attacker).unwrap(),
                )
            })
            .collect(),
    )
}

/// Player 0 declaring attackers.
fn attacks_scene(driver: &Driver) -> String {
    let nobody_there = Answer::DeclareAttackers(vec![(
        ObjectId(u64::MAX),
        AttackTarget::Player(PlayerId(1)),
    )]);
    let answers = vec![
        (
            "only the glass cannon attacks",
            attack(driver, &["Glass Cannon"]),
        ),
        (
            "the wolf and the imp attack: lethal",
            attack(driver, &["Ember Wolf", "Cave Imp"]),
        ),
        (
            "everyone attacks",
            attack(driver, &["Ember Wolf", "Cave Imp", "Glass Cannon"]),
        ),
        ("nobody attacks", attack(driver, &[])),
        ("a creature that is not there", nobody_there),
    ];
    let script = Script {
        seat: PlayerId(0),
        id: DecisionId(11),
        answers,
    };
    render_scene(driver.engine(), script)
}

/// Player 0 attacks with everything and both players pass; player 1 then faces the blocks.
fn blocks_scene(driver: &mut Driver) -> String {
    driver
        .attack(&[("Ember Wolf", 1), ("Cave Imp", 1), ("Glass Cannon", 1)])
        .unwrap();
    for _ in 0..4 {
        if matches!(
            driver.decision().unwrap().kind,
            DecisionKind::DeclareBlockers(_)
        ) {
            break;
        }
        driver.pass().unwrap();
    }
    let driver = &*driver;
    let answers = vec![
        ("no blocks: lethal", block(driver, &[])),
        (
            "the guard trades with the imp",
            block(driver, &[("Stone Guard", "Cave Imp")]),
        ),
        (
            "a double block kills the wolf",
            block(
                driver,
                &[("Stone Guard", "Ember Wolf"), ("Marsh Bear", "Ember Wolf")],
            ),
        ),
        (
            "the guard chumps the cannon",
            block(driver, &[("Stone Guard", "Glass Cannon")]),
        ),
        (
            "two single blocks",
            block(
                driver,
                &[("Stone Guard", "Cave Imp"), ("Marsh Bear", "Glass Cannon")],
            ),
        ),
        (
            "one blocker twice",
            block(
                driver,
                &[("Stone Guard", "Cave Imp"), ("Stone Guard", "Ember Wolf")],
            ),
        ),
    ];
    let script = Script {
        seat: PlayerId(1),
        id: DecisionId(12),
        answers,
    };
    render_scene(driver.engine(), script)
}

fn render() -> String {
    let mut driver = Driver::from_scenario(scenario()).unwrap();
    let attacks = attacks_scene(&driver);
    let blocks = blocks_scene(&mut driver);
    format!(
        "{{\n  \"description\": {},\n  \"blocks\": {blocks},\n  \"attacks\": {attacks}\n}}\n",
        json(&DESCRIPTION),
    )
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(FIXTURE)
}

fn regenerating() -> bool {
    std::env::var_os("MTG_UPDATE_BINDINGS").is_some()
}

#[test]
fn the_committed_fixture_is_what_the_engine_answers() {
    let fresh = render();
    let path = fixture_path();
    if regenerating() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &fresh).unwrap();
        return;
    }
    let committed = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == fresh,
        "the preview fixture drifted; run MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol --test preview_fixture"
    );
}

#[test]
fn the_fixture_holds_replies_of_every_kind_in_the_wire_shape() {
    // Regenerating writes the file in the other test, so judge the fresh text rather than race it.
    let text = if regenerating() {
        render()
    } else {
        fs::read_to_string(fixture_path()).unwrap()
    };
    let fixture: Fixture = serde_json::from_str(&text).unwrap();
    assert!(fixture.description.contains("Regenerate"));

    for (name, scene, kind) in [
        ("blocks", &fixture.blocks, "DeclareBlockers"),
        ("attacks", &fixture.attacks, "DeclareAttackers"),
    ] {
        let ServerMessage::Decision { id, decision } = &scene.decision else {
            panic!("{name}: the scene opens with a decision");
        };
        assert_eq!(decision.player, scene.seat, "{name}");
        assert_eq!(decision.kind.name().replace('-', ""), kind.to_lowercase());
        let mut some = 0;
        let mut none = 0;
        for exchange in &scene.exchanges {
            let ClientMessage::Preview { decision, .. } = &exchange.preview else {
                panic!("{name}: {} previews", exchange.label);
            };
            assert_eq!(decision, id, "{name}: {}", exchange.label);
            let ServerMessage::Forecast { decision, forecast } = &exchange.reply else {
                panic!("{name}: {} is answered with a forecast", exchange.label);
            };
            assert_eq!(decision, id, "{name}: {}", exchange.label);
            if forecast.is_some() {
                some += 1;
            } else {
                none += 1;
            }
        }
        assert!(some >= 3, "{name}: only {some} forecasts");
        assert!(none >= 1, "{name}: no refused answer");
    }

    // The lethal cases the web draws are really in the fixture.
    let lethal = |scene: &Scene| {
        scene.exchanges.iter().any(|e| {
            matches!(&e.reply, ServerMessage::Forecast { forecast: Some(f), .. }
                if f.players.iter().any(|p| p.lethal))
        })
    };
    assert!(lethal(&fixture.blocks) && lethal(&fixture.attacks));
    let deaths = fixture.blocks.exchanges.iter().any(|e| {
        matches!(&e.reply, ServerMessage::Forecast { forecast: Some(f), .. } if f.dying().count() >= 2)
    });
    assert!(deaths, "a block in which two creatures die");
}
