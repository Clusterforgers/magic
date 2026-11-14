//! Goal: the name-sticker words read and write back as the cards print them ("for each name sticker on this Aura with eight or more letters", "... with seven or fewer letters", "choose a letter ... that begins with the chosen letter", "the number of o's in name stickers on ~", "a power and toughness sticker", "another creature you own", "a creature you own that entered this turn"), the two-target block requirement writes as Lineprancers prints it, and every near miss (no number, two letters, another sticker kind, an unknown test) stays with the table.
use super::*;

#[rstest]
#[case::eight_or_more(
    "Enchanted creature gets +0/+2 for each name sticker on this Aura with eight or more letters.",
    ClauseKind::Static,
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":0,"toughness":{"Times":{"by":2,"of":{"NameStickers":{"on":"This","words":{"LettersAtLeast":8}}}}}}}]}}"#
)]
#[case::seven_or_fewer(
    "Enchanted creature gets +2/+0 for each name sticker on this Aura with seven or fewer letters.",
    ClauseKind::Static,
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":{"Times":{"by":2,"of":{"NameStickers":{"on":"This","words":{"LettersAtMost":7}}}}},"toughness":0}}]}}"#
)]
#[case::chosen_letter(
    "Whenever this Vehicle attacks, choose a letter. This Vehicle gets +1/+1 until end of turn for each name sticker on it that begins with the chosen letter.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"ChooseValue":{"kind":"Letter","as":"letter"}},{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":{"NameStickers":{"on":"This","words":{"BeginsWithChosen":"letter"}}},"toughness":{"NameStickers":{"on":"This","words":{"BeginsWithChosen":"letter"}}}}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::letters_in(
    "You gain life equal to the number of o's in name stickers on ~.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"GainLife":{"amount":{"StickerLetters":{"on":"This","letter":"o"}}}}]}}"#
)]
#[case::power_and_toughness(
    "When ~ enters, you get {TK}{TK}, then you may put a power and toughness sticker on a creature you own.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"ticket"},"count":2}},{"May":{"effects":[{"PutSticker":{"on":[{"Type":"Creature"},{"OwnedBy":"You"}],"kind":"PowerToughness"}}]}}]}}"#
)]
#[case::another_you_own(
    "Whenever another creature you own enters, you get {TK}.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"OwnedBy":"You"}],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"ticket"}}}]}}"#
)]
#[case::entered_this_turn(
    "{W}, {T}: You may put a sticker on a creature you own that entered this turn.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{W}"},"Tap"],"effects":[{"May":{"effects":[{"PutSticker":{"on":[{"Type":"Creature"},{"OwnedBy":"You"},"EnteredThisTurn"]}}]}}]}}"#
)]
fn the_name_sticker_words_read(#[case] text: &str, #[case] kind: ClauseKind, #[case] json: &str) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::no_number(
    "Enchanted creature gets +0/+2 for each name sticker on this Aura with many letters.",
    ClauseKind::Static
)]
#[case::words_not_letters(
    "Enchanted creature gets +0/+2 for each name sticker on this Aura with eight or more words.",
    ClauseKind::Static
)]
#[case::two_letters(
    "You gain life equal to the number of ou's in name stickers on ~.",
    ClauseKind::Spell
)]
#[case::art_stickers(
    "You gain life equal to the number of o's in art stickers on ~.",
    ClauseKind::Spell
)]
#[case::a_capital(
    "You gain life equal to the number of O's in name stickers on ~.",
    ClauseKind::Spell
)]
#[case::another_test(
    "~ gets +1/+1 for each name sticker on it that ends with the chosen letter.",
    ClauseKind::Static
)]
#[case::another_owner(
    "Whenever another creature an opponent owns enters, you get {TK}.",
    ClauseKind::Triggered
)]
fn a_name_sticker_near_miss_is_not_read(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}

#[rstest]
#[case::other_than_this(
    r#"{"Activated":{"cost":"{3}{G}","targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}},{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"},{"WithSticker":"PowerToughness"},"Other"]}}],"effects":[{"Rule":{"rule":{"MustBlockAttacker":{"who":{"Target":0},"attacker":{"Target":1}}},"duration":"UntilEndOfTurn"}}]}}"#,
    "{3}{G}: Target creature you don't control blocks target creature you control with a power and toughness sticker on it other than ~ this turn if able."
)]
#[case::any_of_yours(
    r#"{"Activated":{"cost":"{3}{G}","targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}},{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Rule":{"rule":{"MustBlockAttacker":{"who":{"Target":0},"attacker":{"Target":1}}},"duration":"UntilEndOfTurn"}}]}}"#,
    "{3}{G}: Target creature you don't control blocks target creature you control this turn if able."
)]
fn one_target_blocks_the_other_as_printed(#[case] json: &str, #[case] text: &str) {
    let written = render_variants(&ability(json));
    assert!(written.iter().any(|w| same_text(w, text)), "{written:?}");
}

#[rstest]
#[case::eight("Big Ducks", NameWordsCase::AtLeast(8), true)]
#[case::seven("Wicked!", NameWordsCase::AtMost(7), true)]
#[case::too_long("Wonderful", NameWordsCase::AtMost(7), false)]
#[case::begins("zap", NameWordsCase::Begins('Z'), true)]
#[case::begins_after_space(" Zap", NameWordsCase::Begins('z'), true)]
#[case::not_begins("Azz", NameWordsCase::Begins('z'), false)]
fn the_letters_are_counted_without_spaces_or_marks(
    #[case] words: &str,
    #[case] test: NameWordsCase,
    #[case] holds: bool,
) {
    use crate::ability::NameWords;
    let (test, chosen) = match test {
        NameWordsCase::AtLeast(n) => (NameWords::LettersAtLeast(n), None),
        NameWordsCase::AtMost(n) => (NameWords::LettersAtMost(n), None),
        NameWordsCase::Begins(c) => (NameWords::BeginsWithChosen("letter".into()), Some(c)),
    };
    assert_eq!(test.holds(words, chosen), holds);
    assert_eq!(NameWords::letter_count("Ouroboros", 'o'), 4);
}

/// A test of `NameWords` as the table names it.
#[derive(Debug, Clone, Copy)]
enum NameWordsCase {
    AtLeast(u32),
    AtMost(u32),
    Begins(char),
}
