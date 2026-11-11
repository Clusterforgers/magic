//! Goal: the pieces of ep23 translate as the card prints them: lands of every zone ("Lands you control and land cards you own that aren't on the battlefield are Islands") list every zone, a player gains hexproof from colours, and a tap for one type of mana names the type, and a token enters blocking the attacker of a block trigger; creatures in that wording, lands of every player, a player's hexproof from another quality a tap for a type Forge words otherwise and a token blocking another creature stay gaps.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn tap_trigger(produced: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nT:Mode$ TapsForMana | ValidCard$ Permanent | Activator$ You | {produced}Execute$ Add | TriggerZones$ Battlefield | Static$ True | TriggerDescription$ Whenever you tap a permanent for {{C}}, add an additional {{C}}.\nSVar:Add:DB$ Mana | Produced$ C\n"
    )
}

#[rstest]
#[case::colorless("Produced$ C | ", Some("Colorless"))]
#[case::green("Produced$ G | ", Some("Green"))]
#[case::any_mana("", None)]
fn ep23_a_tap_for_one_type_names_it(#[case] produced: &str, #[case] kind: Option<&str>) {
    let script = tap_trigger(produced);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let when = &abilities(&script)[0]["Triggered"]["when"]["TappedForMana"];
    assert_eq!(
        when.get("produced").and_then(|v| v.as_str()),
        kind,
        "{when}"
    );
}

#[test]
fn ep23_a_tap_for_a_combination_stays_a_gap() {
    let found = gaps(&tap_trigger("Produced$ Combo Any | "));
    assert!(
        found
            .iter()
            .any(|gap| gap == "key:Trigger.TapsForMana.Produced=Combo Any"),
        "{found:?}"
    );
}

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
    every_zone_gap(line);
}

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Instant\nA:SP$ {line}\n")
}

#[rstest]
#[case::two_colours("Hexproof:Black & Hexproof:Blue", &["Black", "Blue"])]
#[case::one_colour("Hexproof:Green", &["Green"])]
fn ep23_a_player_gains_hexproof_from_colours(#[case] keywords: &str, #[case] colours: &[&str]) {
    let script = spell(&format!("Pump | Defined$ You | KW$ {keywords}"));
    assert_eq!(gaps(&script), Vec::<String>::new());
    let expected: Vec<_> = colours
        .iter()
        .map(|colour| {
            json!({"Rule": {"duration": "UntilEndOfTurn", "rule": {"PlayersHave": {
                "keyword": {"HexproofFrom": [{"Color": colour}]}, "who": "You"}}}})
        })
        .collect();
    assert_eq!(abilities(&script)[0]["Spell"]["effects"], json!(expected));
}

#[rstest]
#[case::a_card_type("Hexproof:Artifact", "key:Pump.player-keyword-Hexproof:Artifact")]
#[case::a_chosen_colour("Hexproof:ChosenColor", "key:Pump.player-keyword-Hexproof:ChosenColor")]
fn ep23_a_player_s_hexproof_from_other_qualities_stays_a_gap(
    #[case] keyword: &str,
    #[case] gap: &str,
) {
    let found = gaps(&spell(&format!("Pump | Defined$ You | KW$ {keyword}")));
    assert!(found.iter().any(|found| found == gap), "{found:?}");
}

fn every_zone_gap(line: &str) {
    let found = gaps(&static_line(line));
    assert!(
        found
            .iter()
            .any(|gap| gap == "key:Static.Continuous.AffectedZone=All"),
        "{found:?}"
    );
}

fn block_trigger(blocking: &str) -> String {
    format!(
        "Name:T\nTypes:Creature\nPT:3/4\nT:Mode$ AttackerBlocked | ValidCard$ Creature | ValidBlocker$ Card.Self | Execute$ Make | TriggerDescription$ Whenever CARDNAME blocks a creature, create a 1/1 white Soldier creature token that's blocking that creature.\nSVar:Make:DB$ Token | TokenScript$ w_1_1_soldier | TokenOwner$ You | TokenBlocking$ {blocking}\n"
    )
}

#[test]
fn ep23_a_token_enters_blocking_the_attacker() {
    let script = block_trigger("TriggeredAttacker");
    assert_eq!(gaps(&script), Vec::<String>::new());
    let create = &abilities(&script)[0]["Triggered"]["effects"][0]["CreateToken"];
    assert_eq!(create["blocking"], json!("Triggering"), "{create}");
}

#[test]
fn ep23_a_token_blocking_another_creature_stays_a_gap() {
    let found = gaps(&block_trigger("TriggeredBlocker"));
    assert!(
        found
            .iter()
            .any(|gap| gap == "key:TokenBlocking=TriggeredBlocker"),
        "{found:?}"
    );
}
