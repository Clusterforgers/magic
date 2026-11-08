//! Goal: a keyword the card has as long as a counter is on it or it is modified is written back from the rule's selector form as well as from the counter count, and a kind of object other than the card or its host stays unwritten.
use super::*;

#[rstest]
#[case::counter(
    r#"{"Static":{"affected":"This","condition":{"Matches":{"selector":[{"HasCounter":"+1/+1"}],"subject":"This"}},"mods":[{"AddKeyword":"Trample"}]}}"#,
    "~ has trample as long as it has a +1/+1 counter on it"
)]
#[case::modified(
    r#"{"Static":{"affected":"This","condition":{"Matches":{"selector":["Modified"],"subject":"This"}},"mods":[{"AddKeyword":"Flying"}]}}"#,
    "~ has flying as long as it's modified"
)]
fn w19c_a_keyword_with_a_counter_or_modified_condition_is_written(
    #[case] json: &str,
    #[case] text: &str,
) {
    let wordings = render_variants(&ability(json));
    assert!(
        wordings.iter().any(|written| same_text(written, text)),
        "{text}: {wordings:?}"
    );
}

#[rstest]
#[case::the_triggering_object(
    r#"{"Static":{"affected":"This","condition":{"Matches":{"selector":[{"HasCounter":"+1/+1"}],"subject":"Triggering"}},"mods":[{"AddKeyword":"Trample"}]}}"#
)]
#[case::another_counter_than_asked(
    r#"{"Static":{"affected":"This","condition":{"Matches":{"selector":[{"HasCounter":"+1/+1"},"Tapped"],"subject":"This"}},"mods":[{"AddKeyword":"Trample"}]}}"#
)]
fn w19c_other_conditions_stay_unwritten(#[case] json: &str) {
    assert!(render_variants(&ability(json)).is_empty());
}
