//! Goal: a search of the zone outside the game reads back as the wording of the wishes and of "put a card you own from
//! outside the game into your hand", with its reveal, its type words and its optional form, and a wrong filter, owner or
//! destination never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// A wish: the search outside the game for `filter`, the move into the hand and the exile of the card.
fn wish(filter: &str, reveal: bool) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Search":{{"as":"found","filter":{filter},"reveal":{reveal},"zone":"Outside"}}}},{{"Move":{{"to":"Hand","what":{{"Bound":"found"}}}}}},{{"Exile":{{"what":"This"}}}}]}}}}"#
    )
}

#[rstest]
#[case::a_sorcery(
    r#"[{"Type":"Sorcery"},{"OwnedBy":"You"}]"#,
    true,
    "You may reveal a sorcery card you own from outside the game and put it into your hand. Exile ~."
)]
#[case::an_instant(
    r#"[{"Type":"Instant"},{"OwnedBy":"You"}]"#,
    true,
    "You may reveal an instant card you own from outside the game and put it into your hand. Exile ~."
)]
#[case::two_types(
    r#"[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]},{"OwnedBy":"You"}]"#,
    true,
    "You may reveal an artifact or enchantment card you own from outside the game and put it into your hand. Exile ~."
)]
#[case::multicolored(
    r#"[{"Compare":{"op":"Ge","stat":"Colors","to":2}},{"OwnedBy":"You"}]"#,
    true,
    "You may reveal a multicolored card you own from outside the game and put it into your hand. Exile ~."
)]
#[case::any_card_without_a_reveal(
    r#"[{"OwnedBy":"You"}]"#,
    false,
    "You may put a card you own from outside the game into your hand. Exile ~."
)]
fn a_wish_reads_back_as_the_card_words_it(
    #[case] filter: &str,
    #[case] reveal: bool,
    #[case] clause: &str,
) {
    assert!(written(&wish(filter, reveal), clause), "{clause}");
}

#[test]
fn a_named_card_under_a_may_is_worded_after_the_trigger() {
    let legion = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Search":{"as":"found","filter":[{"OwnedBy":"You"},{"Named":"Legion Angel"}],"reveal":true,"zone":"Outside"}},{"Move":{"to":"Hand","what":{"Bound":"found"}}}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
    assert!(written(
        legion,
        "When ~ enters, you may reveal a card you own named Legion Angel from outside the game and put it into your hand."
    ));
}

#[rstest]
#[case::another_type(
    r#"[{"Type":"Sorcery"},{"OwnedBy":"You"}]"#,
    "You may reveal an instant card you own from outside the game and put it into your hand. Exile ~."
)]
#[case::a_reveal_the_card_does_not_say(
    r#"[{"Type":"Sorcery"},{"OwnedBy":"You"}]"#,
    "You may put a sorcery card you own from outside the game into your hand. Exile ~."
)]
#[case::from_your_library(
    r#"[{"Type":"Sorcery"},{"OwnedBy":"You"}]"#,
    "You may reveal a sorcery card you own from your library and put it into your hand. Exile ~."
)]
#[case::onto_the_battlefield(
    r#"[{"Type":"Sorcery"},{"OwnedBy":"You"}]"#,
    "You may reveal a sorcery card you own from outside the game and put it onto the battlefield. Exile ~."
)]
fn a_wrong_type_zone_or_destination_does_not_read_back(#[case] filter: &str, #[case] clause: &str) {
    assert!(!written(&wish(filter, true), clause), "{clause}");
}
