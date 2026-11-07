//! Goal: "transformed", "double-faced" and "prepared" before a noun read back as their predicates, and a selector with another of them, or with none, never reads back as the one the clause says.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// A static ability that gives trample to the other permanents you control that match `pred`.
fn others_have_trample(pred: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{pred},"Other",{{"ControlledBy":"You"}}]}},"mods":[{{"AddKeyword":"Trample"}}]}}}}"#
    )
}

#[rstest]
#[case::transformed(
    r#""Transformed""#,
    "Other transformed permanents you control have trample."
)]
#[case::double_faced(
    r#""DoubleFaced""#,
    "Other double-faced permanents you control have trample."
)]
#[case::prepared(r#""Prepared""#, "Other prepared permanents you control have trample.")]
fn a_face_fact_reads_back_as_its_adjective(#[case] pred: &str, #[case] clause: &str) {
    assert!(written(&others_have_trample(pred), clause), "{clause}");
}

#[rstest]
#[case::double_faced_for_transformed(
    r#""DoubleFaced""#,
    "Other transformed permanents you control have trample."
)]
#[case::transformed_for_double_faced(
    r#""Transformed""#,
    "Other double-faced permanents you control have trample."
)]
#[case::tapped_for_prepared(r#""Tapped""#, "Other prepared permanents you control have trample.")]
#[case::not_transformed(
    r#"{"Not":"Transformed"}"#,
    "Other transformed permanents you control have trample."
)]
fn another_fact_never_reads_back_as_the_adjective(#[case] pred: &str, #[case] clause: &str) {
    assert!(!written(&others_have_trample(pred), clause), "{clause}");
}
