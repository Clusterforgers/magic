//! Goal: "for each A and each B" adds the two counts, "foretold card you own in exile" selects the foretold cards, "creatures that don't have a name" is `Not(HasName)`, "card with foretell" is the named keyword and "Spend this mana only to foretell cards or cast spells that have foretell" restricts mana to both uses; each reads and is written back as the card's text, and a near miss stays with the table.
use super::*;

const FORETOLD: &str = r#"{"Count":["Foretold",{"In":"Exile"},{"OwnedBy":"You"}]}"#;
const HAND: &str = r#"{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}"#;

/// A chapter ability, read by the Saga reader as the corpus card holds it, is written back as its line.
fn written_as(text: &str, json: &str) {
    let found = written_variants(&ability(json));
    assert!(
        found.iter().any(|w| same_text(w, text)),
        "{text}: {found:?}"
    );
}

#[test]
fn r5520_two_counts_add_up() {
    let sum = format!(r#"{{"Sum":[{HAND},{FORETOLD}]}}"#);
    reads_as(
        "~ gets +1/+1 for each card in your hand and each foretold card you own in exile.",
        ClauseKind::Static,
        false,
        &format!(
            r#"{{"Static":{{"affected":"This","mods":[{{"ModifyPT":{{"power":{sum},"toughness":{sum}}}}}]}}}}"#
        ),
    );
}

#[test]
fn r5520_foretold_cards_are_counted() {
    written_as(
        "I — You gain 2 life for each foretold card you own in exile.",
        &format!(
            r#"{{"Triggered":{{"when":{{"Chapter":{{"number":1}}}},"effects":[{{"GainLife":{{"amount":{{"Times":{{"by":2,"of":{FORETOLD}}}}}}}}}]}}}}"#
        ),
    );
}

#[test]
fn r5520_nameless_creatures_cant_block() {
    reads_as(
        "~ can't be blocked by creatures that don't have a name.",
        ClauseKind::Static,
        false,
        r#"{"Static":{"affected":"This","mods":[{"CantBeBlockedBy":[{"Type":"Creature"},{"Not":"HasName"}]}]}}"#,
    );
}

#[test]
fn r5520_a_card_with_foretell_is_returned() {
    written_as(
        "III — Return target card with foretell from your graveyard to your hand.",
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"targets":[{"what":{"Object":[{"HasKeywordNamed":"Foretell"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#,
    );
}

#[test]
fn r5520_mana_for_foretelling() {
    written_as(
        "II — Add {W}{U}. Spend this mana only to foretell cards or cast spells that have foretell.",
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"effects":[{"AddMana":{"mana":{"Fixed":["White","Blue"]},"only":[{"Special":"Foretell"},{"Spell":{"facts":["Foretell"]}}]}}]}}"#,
    );
}

#[rstest]
#[case::and_a_player("~ gets +1/+1 for each card in your hand and each opponent loses 1 life.")]
#[case::foretold_anywhere("~ gets +1/+1 for each foretold card.")]
#[case::named_creatures("~ can't be blocked by creatures that have names.")]
fn r5520_near_misses_stay_with_the_table(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    assert!(
        read.as_ref().is_none_or(|ability| {
            !render_variants(ability)
                .iter()
                .any(|written| same_text(written, text))
        }),
        "{text}: {read:?}"
    );
}

#[rstest]
#[case::foretell_only("II — Add {W}{U}. Spend this mana only to foretell cards.")]
#[case::spells_alone(
    "II — Add {W}{U}. Spend this mana only to foretell cards or cast creature spells."
)]
fn r5520_other_mana_rules_are_not_foretelling(#[case] text: &str) {
    let foretelling = ability(
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"effects":[{"AddMana":{"mana":{"Fixed":["White","Blue"]},"only":[{"Special":"Foretell"},{"Spell":{"facts":["Foretell"]}}]}}]}}"#,
    );
    let found = written_variants(&foretelling);
    assert!(!found.iter().any(|w| same_text(w, text)), "{text}");
}
