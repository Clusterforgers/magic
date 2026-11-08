//! Goal: "sources with the chosen name" are judged on the source in every zone, "if a black card was milled this way" is
//! at least one, and "entered from your graveyard" names its owner; every near-miss keeps its old reading.
use rstest::rstest;
use serde_json::Value;

use super::{abilities, gaps};

fn read(text: &str) -> String {
    assert!(gaps(text).is_empty(), "{:?}", gaps(text));
    Value::Array(abilities(text)).to_string()
}

const NEEDLE: &str = "Name:T\nManaCost:1\nTypes:Artifact\nK:ETBReplacement:Other:DBNameCard\nSVar:DBNameCard:DB$ NameCard | Defined$ You | SpellDescription$ As CARDNAME enters, choose a card name.\n";

#[rstest]
#[case::chosen_name_every_zone(
    "Card.NamedCard",
    r#"{"Cant":{"action":{"Activate":["ChosenName"]},"who":"EachPlayer"}}"#
)]
#[case::permanents_stay_subjects("Artifact,Creature", r#""who":{"Each":"#)]
fn a_prohibition_on_activating_names_its_sources(#[case] valid: &str, #[case] wanted: &str) {
    let text = format!(
        "{NEEDLE}S:Mode$ CantBeActivated | ValidCard$ {valid} | ValidSA$ Activated.!ManaAbility | Description$ x\nOracle:x\n"
    );
    let found = read(&text);
    assert!(found.contains(wanted), "{found}");
}

const GRIST: &str = "Name:T\nTypes:Legendary Planeswalker Grist\nLoyalty:3\nA:AB$ Mill | Cost$ AddCounter<1/LOYALTY> | Planeswalker$ True | Defined$ You | NumCards$ 2 | RememberMilled$ True | SubAbility$ DBDraw | SpellDescription$ x\nSVar:DBDraw:DB$ Draw | ConditionDefined$ Remembered | ConditionPresent$ Card.Black | ConditionCompare$ EQ1 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";

#[rstest]
#[case::a_card_was(
    "Mill two cards. Draw a card if a black card was milled this way.",
    "Ge"
)]
#[case::exactly_one(
    "Mill two cards. Draw a card if exactly one black card was milled this way.",
    "Eq"
)]
#[case::no_such_words("Mill two cards. Draw a card.", "Eq")]
fn a_present_count_of_one_is_at_least_one_when_oracle_says_a(
    #[case] oracle: &str,
    #[case] op: &str,
) {
    let found = read(&format!("{GRIST}Oracle:{oracle}\n"));
    assert!(
        found.contains(&format!(r#""op":"{op}","right":1"#)),
        "{found}"
    );
}

#[rstest]
#[case::your_graveyard(
    "When CARDNAME enters from your graveyard, draw a card.",
    "Card.Self",
    true
)]
#[case::a_graveyard(
    "When CARDNAME enters from a graveyard, draw a card.",
    "Card.Self",
    false
)]
#[case::owner_already_named(
    "When a creature enters from your graveyard, draw a card.",
    "Creature.YouOwn",
    false
)]
fn entering_from_your_graveyard_names_the_owner(
    #[case] description: &str,
    #[case] valid: &str,
    #[case] owned: bool,
) {
    let text = format!(
        "Name:T\nManaCost:G\nTypes:Creature Insect\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Graveyard | Destination$ Battlefield | ValidCard$ {valid} | Execute$ TrigDraw | TriggerZones$ Battlefield | TriggerDescription$ {description}\nSVar:TrigDraw:DB$ Draw | Defined$ You\nOracle:{description}\n"
    );
    let found = read(&text);
    assert_eq!(
        found.matches(r#"{"OwnedBy":"You"}"#).count(),
        usize::from(owned || valid.contains("YouOwn")),
        "{found}"
    );
}
