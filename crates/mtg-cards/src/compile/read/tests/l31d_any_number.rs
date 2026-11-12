//! Goal: "sacrifice any number of ..." and "discard any number of ..." read as the step that binds what it moved, the
//! amounts after it ("that many", "for each ... this way") read as the count of it, and both write back as the card's
//! text; near misses stay with the table.
use super::*;

/// Prints what each clause of `L31D_PROBE` (separated by `|`, kind by a leading `S:`, `T:` or `A:`) reads as and how
/// it is written back; a tool.
#[test]
fn l31d_probe() {
    let list = std::env::var("L31D_PROBE").unwrap_or_default();
    for text in list.split('|').filter(|text| !text.is_empty()) {
        let (kind, text) = match text.split_at(2) {
            ("T:", rest) => (ClauseKind::Triggered, rest),
            ("A:", rest) => (ClauseKind::Activated, rest),
            ("X:", rest) => (ClauseKind::Spell, rest),
            _ => (ClauseKind::Spell, &text[2..]),
        };
        let read = clause(text, kind, text.contains('X'));
        println!(
            "== {text}\n  read: {}",
            read.as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_default())
                .unwrap_or_default()
        );
        if let Some(read) = read {
            for written in written_variants(&read) {
                println!("  wrote: {written}");
            }
        }
    }
}

const SACRIFICED: &str = r#"{"Bound":"sacrificed~moved"}"#;

#[rstest]
#[case::that_much_damage(
    "Sacrifice any number of creatures. ~ deals that much damage to any target.",
    ClauseKind::Spell,
    &format!(r#"{{"Spell":{{"targets":[{{"what":"AnyTarget"}}],"effects":[{{"SacrificeUpTo":{{"filter":[{{"Type":"Creature"}}],"count":{{"Count":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}},"as":"sacrificed"}}}},{{"DealDamage":{{"to":{{"Target":0}},"amount":{SACRIFICED}}}}}]}}}}"#)
)]
#[case::for_each_permanent(
    "Sacrifice any number of permanents. You gain 2 life for each permanent sacrificed this way.",
    ClauseKind::Spell,
    &format!(r#"{{"Spell":{{"effects":[{{"SacrificeUpTo":{{"filter":[],"count":{{"Count":[{{"ControlledBy":"You"}}]}},"as":"sacrificed"}}}},{{"GainLife":{{"amount":{{"Times":{{"by":2,"of":{SACRIFICED}}}}}}}}}]}}}}"#)
)]
#[case::and_or(
    "Sacrifice any number of artifacts, creatures, and/or lands. Draw a card for each permanent sacrificed this way.",
    ClauseKind::Spell,
    &format!(r#"{{"Spell":{{"effects":[{{"SacrificeUpTo":{{"filter":[{{"AnyOf":[{{"Type":"Artifact"}},{{"Type":"Creature"}},{{"Type":"Land"}}]}}],"count":{{"Count":[{{"AnyOf":[{{"Type":"Artifact"}},{{"Type":"Creature"}},{{"Type":"Land"}}]}},{{"ControlledBy":"You"}}]}},"as":"sacrificed"}}}},{{"Draw":{{"count":{SACRIFICED}}}}}]}}}}"#)
)]
#[case::you_control(
    "Sacrifice any number of permanents you control, then draw that many cards.",
    ClauseKind::Spell,
    &format!(r#"{{"Spell":{{"effects":[{{"SacrificeUpTo":{{"filter":[{{"ControlledBy":"You"}}],"count":{{"Count":[{{"ControlledBy":"You"}}]}},"as":"sacrificed"}}}},{{"Draw":{{"count":{SACRIFICED}}}}}]}}}}"#)
)]
#[case::other_in_a_trigger(
    "When ~ enters, sacrifice any number of other permanents, then draw that many cards.",
    ClauseKind::Triggered,
    &format!(r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"SacrificeUpTo":{{"filter":["Other"],"count":{{"Count":["Other",{{"ControlledBy":"You"}}]}},"as":"sacrificed"}}}},{{"Draw":{{"count":{SACRIFICED}}}}}]}}}}"#)
)]
#[case::discard_then_draw(
    "Discard any number of cards, then draw that many cards.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"optional":true,"as":"discarded"}},{"Draw":{"count":{"Bound":"discarded~moved"}}}]}}"#
)]
#[case::discard_for_each(
    "Discard any number of cards. Creatures you control get +0/+1 until end of turn for each card discarded this way.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"optional":true,"as":"discarded"}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":0,"toughness":{"Bound":"discarded~moved"}}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn l31d_any_number_reads_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::up_to_two("Sacrifice up to two creatures. ~ deals that much damage to any target.")]
#[case::other_noun(
    "Sacrifice any number of creatures. You gain 2 life for each land sacrificed this way."
)]
#[case::discarded_for_sacrificed(
    "Discard any number of cards. You gain 2 life for each card sacrificed this way."
)]
#[case::already_x("Sacrifice any number of creatures. ~ deals X damage to any target.")]
fn l31d_any_number_near_misses_stay_with_the_table(#[case] text: &str) {
    let read = clause(text, ClauseKind::Spell, false);
    assert!(
        read.as_ref()
            .is_none_or(|read| !written_variants(read).iter().any(|w| same_text(w, text))),
        "{text}: {read:?}"
    );
}

#[rstest]
#[case::count_of_other(
    r#""count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
    r#""count":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}"#
)]
#[case::bounded(
    r#""count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
    r#""count":2"#
)]
#[case::other_name(r#""as":"sacrificed""#, r#""as":"gone""#)]
#[case::opponent(r#"{"SacrificeUpTo":{"#, r#"{"SacrificeUpTo":{"who":"Opponents","#)]
fn l31d_any_number_is_written_only_for_the_whole_step(#[case] from: &str, #[case] to: &str) {
    let text = "Sacrifice any number of creatures. ~ deals that much damage to any target.";
    let json = format!(
        r#"{{"Spell":{{"targets":[{{"what":"AnyTarget"}}],"effects":[{{"SacrificeUpTo":{{"filter":[{{"Type":"Creature"}}],"count":{{"Count":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}},"as":"sacrificed"}}}},{{"DealDamage":{{"to":{{"Target":0}},"amount":{SACRIFICED}}}}}]}}}}"#
    );
    assert!(json.contains(from), "{from}");
    let written = written_variants(&ability(&json.replace(from, to)));
    assert!(!written.iter().any(|w| same_text(w, text)), "{written:?}");
}
