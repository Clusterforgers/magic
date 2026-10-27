//! Goal: the writers of the party cards give the oracle wording of the four roles joined with "and" and "and/or", counters and a grant to one group in a single sentence, and tokens counted by the party, and a wrong list, group, count or player never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn may_cast(roles: &[&str]) -> String {
    let roles: Vec<String> = roles
        .iter()
        .map(|role| format!(r#"{{"Subtype":"{role}"}}"#))
        .collect();
    format!(
        r#"{{"Rule":{{"rule":{{"MayCast":{{"what":[{{"AnyOf":[{}]}}],"from":"Library"}}}}}}}}"#,
        roles.join(",")
    )
}

#[rstest]
#[case::the_four_roles(
    &["Cleric", "Rogue", "Warrior", "Wizard"],
    "You may cast Cleric, Rogue, Warrior, and Wizard spells from the top of your library."
)]
#[case::or_still_reads(
    &["Cleric", "Rogue", "Warrior", "Wizard"],
    "You may cast Cleric, Rogue, Warrior, or Wizard spells from the top of your library."
)]
fn the_four_party_roles_are_joined_with_and(#[case] roles: &[&str], #[case] clause: &str) {
    assert!(written(&may_cast(roles), clause), "{clause}");
}

#[rstest]
#[case::three_roles(
    &["Cleric", "Rogue", "Warrior"],
    "You may cast Cleric, Rogue, Warrior, and Wizard spells from the top of your library."
)]
#[case::other_roles(
    &["Cleric", "Rogue", "Warrior", "Elf"],
    "You may cast Cleric, Rogue, Warrior, and Wizard spells from the top of your library."
)]
#[case::the_roles_without_the_last(
    &["Cleric", "Rogue", "Warrior", "Wizard"],
    "You may cast Cleric, Rogue, and Warrior spells from the top of your library."
)]
#[case::from_the_graveyard(
    &["Cleric", "Rogue", "Warrior", "Wizard"],
    "You may cast Cleric, Rogue, Warrior, and Wizard spells from your graveyard."
)]
fn another_list_of_roles_never_reads_as_the_party(#[case] roles: &[&str], #[case] clause: &str) {
    assert!(!written(&may_cast(roles), clause), "{clause}");
}

fn ally_search(roles: &[&str]) -> String {
    let roles: Vec<String> = roles
        .iter()
        .map(|role| format!(r#"{{"Subtype":"{role}"}}"#))
        .collect();
    format!(
        r#"{{"Activated":{{"cost":"{{1}}","effects":[{{"LookAtTop":{{"count":6,"as":"top"}}}},{{"Choose":{{"from":{{"Each":[{{"In":"Library"}},{{"Is":{{"Bound":"top"}}}},{{"AnyOf":[{}]}}]}},"count":{{"min":0,"max":2}},"as":"kept"}}}},{{"Reveal":{{"what":{{"Bound":"kept"}}}}}},{{"Move":{{"what":{{"Bound":"kept"}},"to":"Hand"}}}},{{"Move":{{"what":{{"Bound":"top"}},"to":{{"Library":{{"place":"Bottom","order":"Random"}}}}}}}}]}}}}"#,
        roles.join(",")
    )
}

#[test]
fn the_party_and_ally_cards_are_joined_with_and_or() {
    let clause = "{1}: Look at the top six cards of your library. You may reveal up to two Cleric, Rogue, Warrior, Wizard, and/or Ally cards from among them and put them into your hand. Put the rest on the bottom of your library in a random order.";
    assert!(written(
        &ally_search(&["Cleric", "Rogue", "Warrior", "Wizard", "Ally"]),
        clause
    ));
    assert!(!written(
        &ally_search(&["Cleric", "Rogue", "Warrior", "Wizard", "Elf"]),
        clause
    ));
    assert!(!written(
        &ally_search(&["Cleric", "Rogue", "Warrior", "Wizard"]),
        clause
    ));
}

fn counters_and_grant(counted: &str, granted: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"BeginCombat","whose":"You"}}}},"effects":[{{"PutCounters":{{"on":{{"Each":[{counted}]}},"counter":"+1/+1"}}}},{{"Continuous":{{"affected":{{"Each":[{granted}]}},"mods":[{{"AddKeyword":"Deathtouch"}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    )
}

const MINE: &str = r#"{"Type":"Creature"},{"ControlledBy":"You"}"#;
const THEIRS: &str = r#"{"Type":"Creature"},{"ControlledBy":"Opponents"}"#;

#[test]
fn counters_on_a_group_and_a_grant_to_those_creatures_are_one_sentence() {
    let clause = "At the beginning of combat on your turn, put a +1/+1 counter on each creature you control and those creatures gain deathtouch until end of turn.";
    assert!(written(&counters_and_grant(MINE, MINE), clause));
    // Another group, a grant to a different group and a sentence of its own do not join.
    assert!(!written(&counters_and_grant(MINE, THEIRS), clause));
    assert!(!written(&counters_and_grant(THEIRS, MINE), clause));
    assert!(!written(&counters_and_grant(THEIRS, THEIRS), clause));
}

fn party_tokens(who: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Kor Warrior","colors":["White"],"types":["Creature"],"subtypes":["Kor","Warrior"],"power":1,"toughness":1}}}},"count":{{"Party":"{who}"}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::for_each(
    "You",
    "When ~ enters, create a 1/1 white Kor Warrior creature token for each creature in your party.",
    true
)]
#[case::where_x(
    "You",
    "When ~ enters, create X 1/1 white Kor Warrior creature tokens, where X is the number of creatures in your party.",
    true
)]
#[case::their_party(
    "Opponents",
    "When ~ enters, create a 1/1 white Kor Warrior creature token for each creature in your party.",
    false
)]
#[case::two_for_each(
    "You",
    "When ~ enters, create two 1/1 white Kor Warrior creature tokens for each creature in your party.",
    false
)]
fn tokens_are_counted_by_the_party(#[case] who: &str, #[case] clause: &str, #[case] read: bool) {
    assert_eq!(written(&party_tokens(who), clause), read, "{clause}");
}
