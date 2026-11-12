//! Goal: the rules of a booster draft added for the Conspiracy cards are written back as their printed sentences, with
//! the card named "this card" or by its name, and a near miss (another variant, face up for face down) is not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::written_variants;
use rstest::rstest;

fn draft(rule: &str) -> Ability {
    serde_json::from_str(&format!(r#"{{"Rule":{{"rule":{{"Draft":{rule}}}}}}}"#)).unwrap()
}

#[rstest]
#[case::face_up(r#""FaceUp""#, "Draft this card face up.", true)]
#[case::remove_down(
    r#"{"RemoveDrafted":{"face_up":false}}"#,
    "As you draft a card, you may remove it from the draft face down.",
    true
)]
#[case::remove_up(
    r#"{"RemoveDrafted":{"face_up":true}}"#,
    "As you draft a card, you may remove it from the draft face up.",
    true
)]
#[case::remove_up_not_down(
    r#"{"RemoveDrafted":{"face_up":true}}"#,
    "As you draft a card, you may remove it from the draft face down.",
    false
)]
#[case::note_name(
    r#""NoteName""#,
    "As you draft a card, you may reveal it, note its name, then turn this card face down.",
    true
)]
#[case::note_types(
    r#""NoteCreatureTypes""#,
    "As you draft a creature card, you may reveal it, note its creature types, then turn ~ face down.",
    true
)]
#[case::types_not_name(
    r#""NoteCreatureTypes""#,
    "As you draft a card, you may reveal it, note its name, then turn this card face down.",
    false
)]
#[case::additional(
    r#""DraftAdditional""#,
    "As you draft a card, you may draft an additional card from that booster pack. If you do, put this card into that booster pack.",
    true
)]
#[case::additional_not_pass(
    r#""DraftAdditional""#,
    "As you draft a card, you may draft an additional card from that booster pack. If you do, turn this card face down, then pass the next booster pack without drafting a card from it.",
    false
)]
#[case::pass_next(
    r#""DraftAdditionalPassNext""#,
    "As you draft a card, you may draft an additional card from that booster pack. If you do, turn this card face down, then pass the next booster pack without drafting a card from it.",
    true
)]
#[case::whole_pack(
    r#""DraftWholePack""#,
    "Instead of drafting a card from a booster pack, you may draft each card in that booster pack, one at a time. If you do, turn this card face down and you can't draft cards for the rest of this draft round.",
    true
)]
#[case::any_pack(
    r#""LookAtAnyPack""#,
    "During the draft, you may turn this card face down. If you do, look at any unopened booster pack in the draft or any booster pack not being looked at by another player.",
    true
)]
#[case::next_drafted(
    r#""LookAtNextDrafted""#,
    "During the draft, you may turn this card face down. If you do, look at the next card drafted by a player of your choice.",
    true
)]
#[case::next_not_any(
    r#""LookAtNextDrafted""#,
    "During the draft, you may turn this card face down. If you do, look at any unopened booster pack in the draft or any booster pack not being looked at by another player.",
    false
)]
fn a_draft_rule_reads_as_its_sentence(
    #[case] rule: &str,
    #[case] printed: &str,
    #[case] written: bool,
) {
    let texts = written_variants(&draft(rule));
    assert_eq!(
        texts.iter().any(|text| text == printed),
        written,
        "{texts:?}"
    );
}
