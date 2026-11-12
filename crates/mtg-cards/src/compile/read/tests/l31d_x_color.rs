//! Goal: "Spend only black mana on X." reads as the color of X in the cost of an activated ability with `{X}` and as a
//! spell line of its own, writes back as the card's text in each color, and near misses stay with the table.
use super::*;

#[rstest]
#[case::crypt_rats(
    "{X}: ~ deals X damage to each creature and each player. Spend only black mana on X.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{X}"},{"XColor":"Black"}],"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"}]},"amount":"X"}},{"DealDamage":{"to":"EachPlayer","amount":"X"}}]}}"#
)]
#[case::crimson_hellkite(
    "{X}, {T}: ~ deals X damage to target creature. Spend only red mana on X.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{X}"},"Tap",{"XColor":"Red"}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}"#
)]
#[case::white_life(
    "{X}, {T}: You gain X life. Spend only white mana on X.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{X}"},"Tap",{"XColor":"White"}],"effects":[{"GainLife":{"amount":"X"}}]}}"#
)]
#[case::spell_line(
    "Spend only black mana on X.",
    ClauseKind::Static,
    r#"{"AdditionalCost":{"cost":[{"XColor":"Black"}],"required":true}}"#
)]
#[case::green_spell_line(
    "Spend only green mana on X.",
    ClauseKind::Static,
    r#"{"AdditionalCost":{"cost":[{"XColor":"Green"}],"required":true}}"#
)]
fn l31d_x_color_reads_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::no_x(
    "{1}, {T}: You gain 1 life. Spend only white mana on X.",
    ClauseKind::Activated
)]
#[case::colorless(
    "{X}, {T}: You gain X life. Spend only colorless mana on X.",
    ClauseKind::Activated
)]
#[case::colored(
    "{X}, {T}: You gain X life. Spend only colored mana on X.",
    ClauseKind::Activated
)]
#[case::two_colors("Spend only black and/or red mana on X.", ClauseKind::Static)]
#[case::on_y("Spend only black mana on Y.", ClauseKind::Static)]
fn l31d_x_color_near_misses_stay_with_the_table(#[case] text: &str, #[case] kind: ClauseKind) {
    let read = clause(text, kind, false);
    assert!(
        read.as_ref()
            .is_none_or(|read| !written_variants(read).iter().any(|w| same_text(w, text))),
        "{text}: {read:?}"
    );
}

#[rstest]
#[case::other_color(r#"{"XColor":"Black"}"#, r#"{"XColor":"Blue"}"#)]
#[case::dropped(r#",{"XColor":"Black"}"#, "")]
fn l31d_x_color_is_written_only_for_its_own_color(#[case] from: &str, #[case] to: &str) {
    let text =
        "{X}: ~ deals X damage to each creature and each player. Spend only black mana on X.";
    let json = r#"{"Activated":{"cost":[{"Mana":"{X}"},{"XColor":"Black"}],"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"}]},"amount":"X"}},{"DealDamage":{"to":"EachPlayer","amount":"X"}}]}}"#;
    let written = written_variants(&ability(&json.replace(from, to)));
    assert!(!written.iter().any(|w| same_text(w, text)), "{written:?}");
}
