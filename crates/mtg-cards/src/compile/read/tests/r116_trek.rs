//! Goal: the Star Trek wordings read into the DSL and write back as the card's own text: "whenever you face a dilemma", "for each dilemma you've faced this turn", "the number of differently named lands you control", "the number of creature types among non-Borg creatures you control" and "assimilate target ..."; a near miss stays with the table.
use super::*;

#[rstest]
#[case::face_a_dilemma(
    "Whenever you face a dilemma, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"R116Dilemma":{"who":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponent_faces_a_dilemma(
    "Whenever an opponent faces a dilemma, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"R116Dilemma":{"who":"Opponents"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::each_dilemma(
    "At the beginning of your end step, create a 1/1 red Officer creature token for each dilemma you've faced this turn.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Officer","colors":["Red"],"types":["Creature"],"subtypes":["Officer"],"power":1,"toughness":1}},"count":{"Tallied":{"who":"You","what":"R116Dilemmas"}}}}]}}"#
)]
#[case::differently_named_lands(
    "~'s power and toughness are each equal to the number of differently named lands you control.",
    ClauseKind::Static,
    r#"{"Static":{"affected":"This","mods":[{"SetPT":{"power":{"Distinct":{"of":[{"Type":"Land"},{"ControlledBy":"You"}],"stat":"Name"}},"toughness":{"Distinct":{"of":[{"Type":"Land"},{"ControlledBy":"You"}],"stat":"Name"}}}}],"defining":true}}"#
)]
fn r116_wordings_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::no_dilemma("Whenever you face, draw a card.")]
#[case::two_dilemmas("Whenever you face two dilemmas, draw a card.")]
#[case::named_lands(
    "~'s power and toughness are each equal to the number of named lands you control."
)]
fn r116_near_misses_stay_with_the_table(#[case] text: &str) {
    let kind = if text.starts_with("Whenever") {
        ClauseKind::Triggered
    } else {
        ClauseKind::Static
    };
    assert_eq!(clause(text, kind, false), None, "{text}");
}

#[rstest]
#[case::from_a_graveyard(
    "When ~ enters, assimilate target creature card from an opponent's graveyard."
)]
#[case::a_creature(
    "Whenever ~ attacks, assimilate target nonartifact creature an opponent controls."
)]
fn r116_assimilate_reads_and_writes_back(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false).unwrap();
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains("R116Assimilate"), "{json}");
    assert!(
        written_variants(&read)
            .iter()
            .any(|written| same_text(written, text)),
        "{json}"
    );
}
