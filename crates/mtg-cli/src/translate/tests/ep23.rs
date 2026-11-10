//! Goal: the pieces of ep23 translate as the card prints them: lands of every zone ("Lands you control and land cards you own that aren't on the battlefield are Islands") list every zone; creatures in that wording and lands of every player stay gaps.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn static_line(line: &str) -> String {
    format!("Name:T\nTypes:Creature\nPT:1/1\nS:{line}\n")
}

const LANDS: &str = "Description$ Lands you control and land cards you own that aren't on the battlefield are Islands in addition to their other types.";

#[test]
fn ep23_lands_of_every_zone_list_the_zones() {
    let script = static_line(&format!(
        "Mode$ Continuous | Affected$ Land.YouCtrl | AffectedZone$ All | AddType$ Island | {LANDS}"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    let zones = json!({"AnyOf": [{"In": "Battlefield"}, {"In": "Stack"}, {"In": "Hand"}, {"In": "Library"}, {"In": "Graveyard"}, {"In": "Exile"}, {"In": "Command"}]});
    assert_eq!(
        abilities(&script)[0]["Static"]["affected"]["Each"],
        json!([{"Type": "Land"}, {"ControlledBy": "You"}, zones])
    );
}

#[rstest]
#[case::creatures(
    "Mode$ Continuous | Affected$ Creature.YouCtrl | AffectedZone$ All | AddType$ Island | Description$ Creatures you control and creature cards you own that aren't on the battlefield are Islands in addition to their other types."
)]
#[case::every_player_s(&format!(
    "Mode$ Continuous | Affected$ Land | AffectedZone$ All | AddType$ Island | {LANDS}"
))]
fn ep23_other_lands_of_every_zone_stay_gaps(#[case] line: &str) {
    let found = gaps(&static_line(line));
    assert!(
        found
            .iter()
            .any(|gap| gap == "key:Static.Continuous.AffectedZone=All"),
        "{found:?}"
    );
}
