//! Goal: the flash permission whose permanent is sacrificed at the next cleanup step reads into the keyword and writes back as the card's own text; a wording with another step, another sacrificer or no sacrifice stays with the table.
use super::*;

const TEXT: &str = "You may cast ~ as though it had flash. If you cast it any time a sorcery couldn't have been cast, the controller of the permanent it becomes sacrifices it at the beginning of the next cleanup step.";

#[test]
fn r6f_flash_sacrifice_reads_and_writes_back() {
    reads_as(
        TEXT,
        ClauseKind::Static,
        false,
        r#"{"Keyword":"FlashSacrifice"}"#,
    );
}

#[rstest]
#[case::end_step(&TEXT.replace("cleanup step", "end step"))]
#[case::you_sacrifice(&TEXT.replace("the controller of the permanent it becomes sacrifices", "you sacrifice"))]
#[case::no_sacrifice("You may cast ~ as though it had flash.")]
fn r6f_flash_sacrifice_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
