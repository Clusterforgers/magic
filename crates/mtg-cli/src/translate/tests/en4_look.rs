//! Goal: "look at the top card of your library" and "play with the top card revealed" lines become the look and reveal
//! rules of the player they name, and a hand, a face-down permanent, a condition or a cast permission stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn card(line: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\nS:Mode$ Continuous | {line}\n")
}

#[rstest]
#[case::look_ctrl("Affected$ Card.TopLibrary+YouCtrl | AffectedZone$ Library | MayLookAt$ You | Description$ x", json!({"LookAtTopAnyTime": "You"}))]
#[case::look_own("Affected$ Card.TopLibrary+YouOwn | AffectedZone$ Library | MayLookAt$ You | Description$ x", json!({"LookAtTopAnyTime": "You"}))]
#[case::reveal_mine("Affected$ Card.TopLibrary+YouCtrl | AffectedZone$ Library | MayLookAt$ Player | Description$ x", json!({"RevealTop": "You"}))]
#[case::reveal_all("Affected$ Card.TopLibrary | AffectedZone$ Library | MayLookAt$ Player | Description$ x", json!({"RevealTop": "EachPlayer"}))]
fn a_look_line_maps(#[case] line: &str, #[case] rule: Value) {
    let script = card(line);
    let mut found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(normal(found.remove(0)), json!({"Rule": {"rule": rule}}));
}

#[rstest]
#[case::hand(
    "AffectedZone$ Hand | MayLookAt$ Player | Description$ x",
    "static:MayLookAt-zone"
)]
#[case::face_down(
    "Affected$ Creature.faceDown+OppCtrl | AffectedZone$ Battlefield | MayLookAt$ You",
    "static:MayLookAt-zone"
)]
#[case::exiled(
    "Affected$ Card.ExiledWithSource | AffectedZone$ Exile | MayLookAt$ You",
    "static:MayLookAt-zone"
)]
#[case::everyone_looks(
    "Affected$ Card.TopLibrary | AffectedZone$ Library | MayLookAt$ You",
    "static:MayLookAt-who"
)]
#[case::only_a_kind(
    "Affected$ Creature.TopLibrary+YouCtrl+nonLand | AffectedZone$ Library | MayLookAt$ You",
    "static:MayLookAt-affected"
)]
#[case::while_solved(
    "Affected$ Card.TopLibrary+YouCtrl | IsPresent$ Card.Self+IsSolved | AffectedZone$ Library | MayLookAt$ You",
    "valid:IsSolved"
)]
fn a_shape_that_does_not_map_is_a_named_gap(#[case] line: &str, #[case] gap: &str) {
    let script = card(line);
    assert_eq!(abilities(&script).len(), 0);
    assert_eq!(gaps(&script), [gap]);
}
