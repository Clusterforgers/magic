//! Goal: mana restricted by a fact of the spell (colorless, from exile, from anywhere other than your hand, devoid,
//! flashback from a graveyard, outlaw, "can't be spent to cast spells from your hand") is written and read as the cards
//! word it, and another fact, zone or kind is not its text.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

/// A tap ability that adds one mana of any color with the uses.
fn any_color(only: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"AnyColor":1}},"only":{only}}}}}]}}}}"#
    )
}

const COLORLESS: &str = r#"[{"Spell":{"facts":["Colorless"]}}]"#;
const COLORLESS_OR_ABILITY: &str = r#"[{"Spell":{"facts":["Colorless"]}},{"Ability":{}}]"#;
const EXILE: &str = r#"[{"Spell":{"facts":["FromExile"]}}]"#;
const ELSEWHERE: &str = r#"[{"Spell":{"facts":["NotFromHand"]}}]"#;
const DEVOID: &str = r#"[{"Spell":{"facts":["Devoid"]}}]"#;
const FLASHBACK: &str = r#"[{"Spell":{"facts":["FromGraveyard","Flashback"]}}]"#;
const OUTLAW: &str = r#"[{"Spell":{"subtypes":["Assassin","Mercenary","Pirate","Rogue","Warlock"]}},{"Ability":{"subtypes":["Assassin","Mercenary","Pirate","Rogue","Warlock"]}}]"#;
const NOT_HAND: &str = r#"[{"Spell":{"facts":["NotFromHand"]}},{"Ability":{}},"OtherCosts"]"#;

const ADD: &str = "{T}: Add one mana of any color.";

#[rstest]
#[case::shrine_of_the_forsaken_gods(COLORLESS, "Spend this mana only to cast colorless spells.")]
#[case::sage_of_the_unknowable(
    COLORLESS_OR_ABILITY,
    "Spend this mana only to cast a colorless spell or to activate an ability."
)]
#[case::interdimensional_web_watch(EXILE, "Spend this mana only to cast spells from exile.")]
#[case::mmmenon(
    ELSEWHERE,
    "Spend this mana only to cast a spell from anywhere other than your hand."
)]
#[case::corrupted_crossroads(DEVOID, "Spend this mana only to cast a spell with devoid.")]
#[case::altar_of_the_lost(
    FLASHBACK,
    "Spend this mana only to cast spells with flashback from a graveyard."
)]
#[case::discreet_retreat(
    OUTLAW,
    "Spend this mana only to cast outlaw spells or activate abilities of outlaw sources."
)]
#[case::vhal(NOT_HAND, "This mana can't be spent to cast spells from your hand.")]
fn a_fact_of_the_spell_is_written_as_the_card_words_it(#[case] only: &str, #[case] sentence: &str) {
    let json = any_color(only);
    let text = format!("{ADD} {sentence}");
    assert!(
        writes(&json, &text),
        "{text}: {:?}",
        render_variants(&ability(&json))
    );
}

#[rstest]
#[case::colorless_for_any_spell(COLORLESS, "Spend this mana only to cast spells.")]
#[case::colorless_without_the_ability(
    COLORLESS_OR_ABILITY,
    "Spend this mana only to cast a colorless spell."
)]
#[case::any_spell_with_the_ability(
    COLORLESS_OR_ABILITY,
    "Spend this mana only to cast a spell or to activate an ability."
)]
#[case::exile_for_a_graveyard(EXILE, "Spend this mana only to cast spells from a graveyard.")]
#[case::elsewhere_for_the_hand(ELSEWHERE, "Spend this mana only to cast a spell from your hand.")]
#[case::devoid_for_flashback(DEVOID, "Spend this mana only to cast a spell with flashback.")]
#[case::flashback_from_anywhere(FLASHBACK, "Spend this mana only to cast spells with flashback.")]
#[case::outlaw_for_a_pirate(
    OUTLAW,
    "Spend this mana only to cast Pirate spells or activate abilities of Pirate sources."
)]
#[case::the_hand_without_abilities(
    NOT_HAND,
    "Spend this mana only to cast a spell from anywhere other than your hand."
)]
fn another_fact_is_not_the_text(#[case] only: &str, #[case] sentence: &str) {
    let json = any_color(only);
    let text = format!("{ADD} {sentence}");
    assert!(!writes(&json, &text), "{text}");
}

#[rstest]
#[case::colorless(COLORLESS, "Spend this mana only to cast colorless spells.")]
#[case::colorless_or_ability(
    COLORLESS_OR_ABILITY,
    "Spend this mana only to cast a colorless spell or to activate an ability."
)]
#[case::exile(EXILE, "Spend this mana only to cast spells from exile.")]
#[case::elsewhere(
    ELSEWHERE,
    "Spend this mana only to cast a spell from anywhere other than your hand."
)]
#[case::devoid(DEVOID, "Spend this mana only to cast a spell with devoid.")]
#[case::outlaw(
    OUTLAW,
    "Spend this mana only to cast outlaw spells or activate abilities of outlaw sources."
)]
#[case::not_hand(NOT_HAND, "This mana can't be spent to cast spells from your hand.")]
fn the_sentence_reads_back_to_the_same_uses(#[case] only: &str, #[case] sentence: &str) {
    let text = format!("{ADD} {sentence}");
    reads_as(&text, ClauseKind::Activated, false, &any_color(only));
}
