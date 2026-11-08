//! Goal: an intervening "if you control another X" (a count that leaves the card itself out) writes with "another" for one kind, a list of kinds and a card type; a count that keeps the card, or a different kind, does not write it.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn enters_if(count: &str, op: &str, number: u32) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Draw":{{}}}}],"if":{{"Compare":{{"left":{{"Count":[{count}]}},"op":"{op}","right":{number}}}}},"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}}}}}}"#
    )
}

#[rstest]
#[case::subtype(
    r#"{"Subtype":"Human"},"Other",{"ControlledBy":"You"}"#,
    "When ~ enters, if you control another Human, draw a card."
)]
#[case::other_last(
    r#"{"Subtype":"Human"},{"ControlledBy":"You"},"Other""#,
    "When ~ enters, if you control another Human, draw a card."
)]
#[case::card_type(
    r#"{"Type":"Creature"},"Other",{"ControlledBy":"You"}"#,
    "When ~ enters, if you control another creature, draw a card."
)]
#[case::two_kinds(
    r#"{"AnyOf":[{"Subtype":"Human"},{"Subtype":"Elf"}]},"Other",{"ControlledBy":"You"}"#,
    "When ~ enters, if you control another Human or Elf, draw a card."
)]
fn control_another(#[case] count: &str, #[case] text: &str) {
    assert!(written(&enters_if(count, "Ge", 1), text));
}

#[rstest]
#[case::the_card_counts(
    r#"{"Subtype":"Human"},{"ControlledBy":"You"}"#,
    "Ge",
    1,
    "When ~ enters, if you control another Human, draw a card."
)]
#[case::an_opponent_controls(
    r#"{"Subtype":"Human"},"Other",{"ControlledBy":"Opponents"}"#,
    "Ge",
    1,
    "When ~ enters, if you control another Human, draw a card."
)]
#[case::another_kind(
    r#"{"Subtype":"Elf"},"Other",{"ControlledBy":"You"}"#,
    "Ge",
    1,
    "When ~ enters, if you control another Human, draw a card."
)]
#[case::no_other_leaves_none(
    r#"{"Subtype":"Human"},"Other",{"ControlledBy":"You"}"#,
    "Eq",
    0,
    "When ~ enters, if you control another Human, draw a card."
)]
fn not_the_same_rule(
    #[case] count: &str,
    #[case] op: &str,
    #[case] number: u32,
    #[case] text: &str,
) {
    assert!(!written(&enters_if(count, op, number), text));
}

fn upkeep_if(left: &str, op: &str, number: u32) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"DealDamage":{{"amount":2,"to":"Triggering"}}}}],"if":{{"Compare":{{"left":{{"Count":[{left}]}},"op":"{op}","right":{number}}}}},"when":{{"StepBegins":{{"step":"Upkeep","whose":"Opponents"}}}}}}}}"#
    )
}

const THAT_PLAYER_TWO: &str = "At the beginning of each opponent's upkeep, if that player has two or fewer cards in hand, ~ deals 2 damage to that player.";

#[rstest]
#[case::owner_first(r#"{"OwnedBy":"ActivePlayer"},{"In":"Hand"}"#, "Le", 2)]
#[case::zone_first(r#"{"In":"Hand"},{"OwnedBy":"ActivePlayer"}"#, "Le", 2)]
fn the_hand_of_the_active_player(#[case] left: &str, #[case] op: &str, #[case] number: u32) {
    assert!(written(&upkeep_if(left, op, number), THAT_PLAYER_TWO));
}

#[rstest]
#[case::more_cards(r#"{"OwnedBy":"ActivePlayer"},{"In":"Hand"}"#, "Le", 3)]
#[case::at_least(r#"{"OwnedBy":"ActivePlayer"},{"In":"Hand"}"#, "Ge", 2)]
#[case::your_hand(r#"{"OwnedBy":"You"},{"In":"Hand"}"#, "Le", 2)]
#[case::the_graveyard(r#"{"OwnedBy":"ActivePlayer"},{"In":"Graveyard"}"#, "Le", 2)]
fn another_hand_count_is_another_rule(#[case] left: &str, #[case] op: &str, #[case] number: u32) {
    assert!(!written(&upkeep_if(left, op, number), THAT_PLAYER_TWO));
}

#[test]
fn damage_to_that_player_may_say_them() {
    let json = upkeep_if(r#"{"OwnedBy":"ActivePlayer"},{"In":"Hand"}"#, "Le", 2);
    assert!(written(
        &json,
        &THAT_PLAYER_TWO.replace("that player.", "them.")
    ));
    assert!(!written(
        &json,
        &THAT_PLAYER_TWO.replace("that player.", "you.")
    ));
}

const YOMIJI: &str = "Whenever a legendary permanent other than ~ is put into a graveyard from the battlefield, return that card to its owner's hand.";

#[rstest]
#[case::other_than_this("Other", true)]
#[case::the_card_counts("Plain", false)]
fn another_permanent_may_say_other_than_this(#[case] other: &str, #[case] reads: bool) {
    let preds = if other == "Other" {
        r#"{"Supertype":"Legendary"},"Other""#
    } else {
        r#"{"Supertype":"Legendary"}"#
    };
    let json = format!(
        r#"{{"Triggered":{{"effects":[{{"Move":{{"to":"Hand","what":"Triggering"}}}}],"when":{{"ZoneChange":{{"from":"Battlefield","to":"Graveyard","what":[{preds}]}}}}}}}}"#
    );
    assert_eq!(written(&json, YOMIJI), reads);
}

fn life_lost(who: &str, number: u32) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Draw":{{}}}}],"if":{{"Compare":{{"left":{{"Tallied":{{"what":"LifeLost","who":"{who}"}}}},"op":"Ge","right":{number}}}}},"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}}}}}}"#
    )
}

#[rstest]
#[case::an_opponent("Opponents", 1, true)]
#[case::two_life("Opponents", 2, false)]
#[case::you("You", 1, false)]
fn an_opponent_lost_life(#[case] who: &str, #[case] number: u32, #[case] reads: bool) {
    assert_eq!(
        written(
            &life_lost(who, number),
            "When ~ enters, if an opponent lost life this turn, draw a card."
        ),
        reads
    );
}
