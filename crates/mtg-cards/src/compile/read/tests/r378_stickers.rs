//! Goal: the sticker kinds read and write back as the cards print them ("put a name sticker on it", "put an art sticker on a nonland permanent you own", "put up to two name stickers on it", "distribute up to two art stickers among one or two ...", "a sticker on a creature card in your graveyard", "with an art sticker on it", "for each name sticker on it"), and every near miss (an unknown kind, a count of one, an unequal pair of numbers, a permanent not owned) stays with the table.
use super::*;

#[rstest]
#[case::name_on_it(
    "When ~ enters, you may put a name sticker on it.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"PutSticker":{"on":["This"],"kind":"Name"}}]}}]}}"#
)]
#[case::art_on_owned(
    "When ~ enters, you may put an art sticker on a nonland permanent you own.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"PutSticker":{"on":[{"Not":{"Type":"Land"}},{"OwnedBy":"You"}],"kind":"Art"}}]}}]}}"#
)]
#[case::up_to_two(
    "When ~ enters, you may put up to two name stickers on it.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"PutSticker":{"on":["This"],"kind":"Name","up_to":2}}]}}]}}"#
)]
#[case::distribute(
    "When ~ enters, distribute up to two art stickers among one or two nonland permanents you own.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"PutSticker":{"on":[{"Not":{"Type":"Land"}},{"OwnedBy":"You"}],"kind":"Art","up_to":2}}]}}"#
)]
#[case::with_art_sticker(
    "{2}{R}: Another target creature with an art sticker on it gets +2/+0 and gains menace until end of turn.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":"{2}{R}","targets":[{"what":{"Object":[{"Type":"Creature"},{"WithSticker":"Art"},"Other"]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":2,"toughness":0}},{"AddKeyword":"Menace"}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn the_sticker_kinds_read(#[case] text: &str, #[case] kind: ClauseKind, #[case] json: &str) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::unknown_kind("When ~ enters, you may put a glitter sticker on it.")]
#[case::up_to_one("When ~ enters, you may put up to one name stickers on it.")]
#[case::unequal(
    "When ~ enters, distribute up to two art stickers among one or three nonland permanents you own."
)]
#[case::not_owned(
    "When ~ enters, you may put a name sticker on a nonland permanent an opponent controls."
)]
fn a_sticker_near_miss_is_not_read(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

#[rstest]
#[case::name(crate::ability::StickerKind::Name, "with a name sticker on it")]
#[case::art(crate::ability::StickerKind::Art, "with an art sticker on it")]
#[case::ability(crate::ability::StickerKind::Ability, "with an ability sticker on it")]
fn a_sticker_kind_writes_as_a_suffix(
    #[case] kind: crate::ability::StickerKind,
    #[case] words: &str,
) {
    let selector = crate::refs::Selector(vec![
        crate::refs::Pred::Type(mtg_core::card::CardType::Creature),
        crate::refs::Pred::WithSticker(kind),
    ]);
    let text =
        super::super::selectors::phrase_text(&selector, super::super::selectors::Form::Target);
    assert_eq!(text, Some(format!("creature {words}")));
}
