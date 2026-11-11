//! Goal: the writers of ep27 write the card's words and nothing near them. "You may cast this spell as though it had flash if C." and the forms with the condition first ("As long as C, ...", "If C, ...") read as a `MayCast` rule of the card from its hand and write back; a permission from another zone, without flash, for other cards or without its condition does not.
use super::*;

const FAERIE: &str = r#"{"Compare":{"left":{"Count":[{"Subtype":"Faerie"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#;

fn self_flash(condition: &str, from: &str, permission: &str) -> Ability {
    ability(&format!(
        r#"{{"Rule":{{"rule":{{"MayCast":{permission}}},"condition":{condition},"from":"{from}"}}}}"#
    ))
}

const FLASH: &str = r#"{"what":["This"],"flash":true}"#;
const SPINNERS: &str = "You may cast ~ as though it had flash if you control a Faerie.";

#[rstest]
#[case::spinners(SPINNERS)]
#[case::condition_first("As long as you control a Faerie, you may cast ~ as though it had flash.")]
#[case::if_first("If you control a Faerie, you may cast ~ as though it had flash.")]
fn ep27_self_flash_reads_and_writes(#[case] text: &str) {
    let read = golden(text, ClauseKind::Static, false);
    assert_eq!(read, self_flash(FAERIE, "Hand", FLASH), "{text}");
}

#[rstest]
#[case::spinners(self_flash(FAERIE, "Hand", FLASH), SPINNERS, true)]
#[case::from_the_graveyard(self_flash(FAERIE, "Graveyard", FLASH), SPINNERS, false)]
#[case::without_flash(self_flash(FAERIE, "Hand", r#"{"what":["This"]}"#), SPINNERS, false)]
#[case::other_cards(
    self_flash(FAERIE, "Hand", r#"{"what":[{"Type":"Creature"}],"flash":true}"#),
    SPINNERS,
    false
)]
#[case::free(
    self_flash(FAERIE, "Hand", r#"{"what":["This"],"flash":true,"free":true}"#),
    SPINNERS,
    false
)]
#[case::another_condition(
    self_flash(&FAERIE.replace("Faerie", "Human"), "Hand", FLASH),
    SPINNERS,
    false
)]
#[case::without_a_condition(
    ability(r#"{"Rule":{"rule":{"MayCast":{"what":["This"],"flash":true}},"from":"Hand"}}"#),
    "You may cast ~ as though it had flash.",
    false
)]
fn ep27_self_flash_near_misses_differ(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}
