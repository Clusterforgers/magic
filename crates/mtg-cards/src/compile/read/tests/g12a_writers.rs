//! Goal: the round 12 writers give the oracle wording of a card that goes into its owner's library instead of a graveyard (shuffled in after a reveal, on top, on the bottom), and a rule of another meaning (other place, no reveal, only from the battlefield, another object) never reads back as the clause.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn into_library(place: &str, reveal: bool, from: &str, what: &str) -> String {
    let reveal = if reveal { r#","reveal":true"# } else { "" };
    format!(
        r#"{{"Replacement":{{"by":{{"IntoLibrary":{{"place":"{place}"{reveal}}}}},"when":{{"ZoneChange":{{{from}"to":"Graveyard","what":{what}}}}}}}}}"#
    )
}

const THIS: &str = r#"["This"]"#;
const DIES: &str = r#""from":"Battlefield","#;
const COLOSSUS: &str = "If ~ would be put into a graveyard from anywhere, reveal ~ and shuffle it into its owner's library instead.";

#[rstest]
#[case::shuffled(into_library("Shuffled", true, "", THIS), true)]
#[case::not_revealed(into_library("Shuffled", false, "", THIS), false)]
#[case::on_top(into_library("Top", true, "", THIS), false)]
#[case::from_the_battlefield(into_library("Shuffled", true, DIES, THIS), false)]
#[case::another_card(into_library("Shuffled", true, "", r#"[{"Type":"Creature"}]"#), false)]
fn g12a_shuffled_in_from_anywhere(#[case] json: String, #[case] reads: bool) {
    assert_eq!(written(&json, COLOSSUS), reads, "{json}");
}

#[rstest]
#[case::top_by_name(
    "If ~ would die, put ~ on top of its owner's library instead.",
    "Top",
    true
)]
#[case::bottom(
    "If ~ would die, put it on the bottom of its owner's library instead.",
    "Bottom",
    true
)]
#[case::top_read_as_bottom(
    "If ~ would die, put ~ on top of its owner's library instead.",
    "Bottom",
    false
)]
#[case::bottom_read_as_shuffle(
    "If ~ would die, put it on the bottom of its owner's library instead.",
    "Shuffled",
    false
)]
fn g12a_a_dying_card_goes_on_top_or_bottom(
    #[case] clause: &str,
    #[case] place: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        written(&into_library(place, false, DIES, THIS), clause),
        reads
    );
}
