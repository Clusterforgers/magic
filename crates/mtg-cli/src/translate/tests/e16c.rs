//! Goal: round 16 pack e16c reads tokens made attacking the player an attack trigger went at, "the last counter is
//! removed" and counters a player gets together, and leaves every other defender, amount and counter trigger a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with these lines as compact JSON, and its gaps.
fn card(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Enchantment\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const TOKEN: &str =
    "SVar:TrigToken:DB$ Token | TokenScript$ w_1_1_soldier | TokenTapped$ True | TokenAttacking$ ";

#[rstest]
#[case::you_attack_a_player(
    "T:Mode$ AttackersDeclaredOneTarget | ValidAttackers$ Creature.YouCtrl | AttackedTarget$ Player | Execute$ TrigToken | TriggerZones$ Battlefield",
    "TriggeredAttackedTarget",
    "Against"
)]
#[case::equipped_attacks(
    "T:Mode$ Attacks | ValidCard$ Card.AttachedBy | Attacked$ Player.withMostLife | Execute$ TrigToken",
    "TriggeredDefender",
    "Defending"
)]
fn a_token_attacks_the_player_attacked(
    #[case] trigger: &str,
    #[case] word: &str,
    #[case] against: &str,
) {
    let (json, found) = card(&format!("{trigger}\n{TOKEN}{word}"));
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(&format!(r#""against":"{against}","attacking":true"#)),
        "{json}"
    );
}

#[rstest]
#[case::remembered(
    "T:Mode$ AttackersDeclaredOneTarget | ValidAttackers$ Creature.YouCtrl | AttackedTarget$ Player | Execute$ TrigToken | TriggerZones$ Battlefield",
    "Remembered"
)]
#[case::outside_an_attack(
    "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigToken",
    "TriggeredAttackedTarget"
)]
fn another_defender_stays_a_gap(#[case] trigger: &str, #[case] word: &str) {
    let (_, found) = card(&format!("{trigger}\n{TOKEN}{word}"));
    assert!(
        found
            .iter()
            .any(|gap| gap == &format!("key:TokenAttacking={word}")),
        "{found:?}"
    );
}

const LAST: &str = "T:Mode$ CounterRemoved | ValidCard$ Card.Self | TriggerZones$ Exile | CounterType$ TIME | Execute$ TrigDraw | NewCounterAmount$ AMOUNT\nSVar:TrigDraw:DB$ Draw";

#[rstest]
#[case::none_left("0", "Eq", 0)]
#[case::compared("LE1", "Le", 1)]
fn the_counters_left_are_compared_as_it_triggers(
    #[case] amount: &str,
    #[case] op: &str,
    #[case] right: u32,
) {
    let (json, found) = card(&LAST.replace("AMOUNT", amount));
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(&format!(
            r#""if":{{"AtTrigger":{{"Compare":{{"left":{{"Counters":{{"counter":{{"Other":"time"}},"on":"This"}}}},"op":"{op}","right":{right}}}}}}}"#
        )),
        "{json}"
    );
}

#[rstest]
#[case::another_object(LAST.replace("AMOUNT", "0").replace("Card.Self", "Creature.YouCtrl"))]
#[case::unread_amount(LAST.replace("AMOUNT", "X"))]
fn another_counters_left_stays_a_gap(#[case] lines: String) {
    let (_, found) = card(&lines);
    assert!(!found.is_empty(), "{lines}");
}

const ENERGY: &str = "T:Mode$ CounterAddedOnce | ValidPlayer$ WHO | TriggerZones$ Battlefield | CounterType$ ENERGY | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw";

#[rstest]
#[case::you("You", "You")]
#[case::an_opponent("Opponent", "Opponents")]
fn counters_a_player_gets_together_trigger_once(#[case] word: &str, #[case] who: &str) {
    let (json, found) = card(&ENERGY.replace("WHO", word));
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(&format!(
            r#""when":{{"Batch":{{"PlayerCountersPut":{{"counter":{{"Other":"energy"}},"who":"{who}"}}}}}}"#
        )),
        "{json}"
    );
}

#[rstest]
#[case::unknown_player(ENERGY.replace("WHO", "Player.withMostLife"))]
#[case::first_time(ENERGY.replace("WHO", "You").replace("| Execute", "| FirstTime$ True | Execute"))]
fn another_player_counter_trigger_stays_a_gap(#[case] lines: String) {
    let (_, found) = card(&lines);
    assert!(!found.is_empty(), "{lines}");
}
