//! Goal: the writers of ep31 write the card's words and nothing near them. A token copy of a named card ("Create a Tarmogoyf token", `TokenRef::Card`) writes the name with its count and tapped state; another name, a described token or a copy of an object does not. What abilities an object has (`Pred::HasAbility`: "with no abilities", "with a mana ability", "without mana abilities", "with an activated ability that isn't a mana ability") reads and writes back; the other kind or its negation does not. "Each player shuffles their hand and graveyard into their library, then draws seven cards" writes as one sentence; another count or player does not.
use super::*;

fn card_token(token: &str, extra: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"CreateToken":{{"token":{token}{extra}}}}}]}}}}"#
    ))
}

const GOYF: &str = r#"{"Card":"Tarmogoyf"}"#;

#[rstest]
#[case::one(card_token(GOYF, ""), "{T}: Create a Tarmogoyf token.", true)]
#[case::an_article(
    card_token(r#"{"Card":"Akroma, Angel of Wrath"}"#, ""),
    "{T}: Create an Akroma, Angel of Wrath token.",
    true
)]
#[case::ten(
    card_token(r#"{"Card":"Scryb Sprites"}"#, r#","count":10"#),
    "{T}: Create ten Scryb Sprites tokens.",
    true
)]
#[case::tapped(
    card_token(r#"{"Card":"Mutavault"}"#, r#","tapped":true"#),
    "{T}: Create a tapped Mutavault token.",
    true
)]
#[case::another_name(card_token(GOYF, ""), "{T}: Create a Lhurgoyf token.", false)]
#[case::two_are_not_one(
    card_token(GOYF, r#","count":2"#),
    "{T}: Create a Tarmogoyf token.",
    false
)]
#[case::untapped_is_not_tapped(
    card_token(GOYF, ""),
    "{T}: Create a tapped Tarmogoyf token.",
    false
)]
#[case::a_copy_of_an_object_is_not_the_card(
    card_token(r#"{"CopyOf":{"of":"This"}}"#, ""),
    "{T}: Create a Tarmogoyf token.",
    false
)]
fn ep31_a_token_copy_of_a_named_card(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

fn pumped(pred: &str) -> Ability {
    ability(&format!(
        r#"{{"Static":{{"mods":[{{"ModifyPT":{{"power":2,"toughness":2}}}}],"affected":{{"Each":[{{"Type":"Creature"}},{pred}]}}}}}}"#
    ))
}

const NONE: &str = r#"{"Not":{"HasAbility":"Any"}}"#;
const MANA: &str = r#"{"HasAbility":"Mana"}"#;
const NO_MANA: &str = r#"{"Not":{"HasAbility":"Mana"}}"#;
const ACTIVATED: &str = r#"{"HasAbility":"NonManaActivated"}"#;

#[rstest]
#[case::no_abilities(NONE, "Creatures with no abilities get +2/+2.")]
#[case::a_mana_ability(MANA, "Creatures with a mana ability get +2/+2.")]
#[case::without_mana_abilities(NO_MANA, "Creatures without mana abilities get +2/+2.")]
#[case::activated(
    ACTIVATED,
    "Creatures with an activated ability that isn't a mana ability get +2/+2."
)]
fn ep31_abilities_an_object_has_read_and_write(#[case] pred: &str, #[case] text: &str) {
    assert_eq!(golden(text, ClauseKind::Static, false), pumped(pred));
    let found = written_variants(&pumped(pred));
    assert!(found.iter().any(|w| same_text(w, text)), "{found:?}");
}

#[rstest]
#[case::any_is_not_mana(r#"{"HasAbility":"Any"}"#, "Creatures with a mana ability get +2/+2.")]
#[case::mana_is_not_its_negation(MANA, "Creatures without mana abilities get +2/+2.")]
#[case::none_is_not_mana(NONE, "Creatures without mana abilities get +2/+2.")]
#[case::activated_is_not_mana(ACTIVATED, "Creatures with a mana ability get +2/+2.")]
fn ep31_abilities_near_misses_differ(#[case] pred: &str, #[case] text: &str) {
    let found = written_variants(&pumped(pred));
    assert!(!found.iter().any(|w| same_text(w, text)), "{found:?}");
}

fn twister(who: &str, count: u32) -> Ability {
    ability(&format!(
        r#"{{"Spell":{{"effects":[{{"Move":{{"what":{{"Each":[{{"AnyOf":[{{"In":"Hand"}},{{"In":"Graveyard"}}]}}]}},"to":{{"Library":{{"place":"Shuffled"}}}}}}}},{{"Draw":{{"who":"{who}","count":{count}}}}}]}}}}"#
    ))
}

const TWISTER: &str =
    "Each player shuffles their hand and graveyard into their library, then draws seven cards.";

#[rstest]
#[case::seven(twister("EachPlayer", 7), TWISTER, true)]
#[case::six_is_not_seven(twister("EachPlayer", 6), TWISTER, false)]
#[case::you_are_not_each_player(twister("You", 7), TWISTER, false)]
fn ep31_each_player_shuffles_then_draws(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}
