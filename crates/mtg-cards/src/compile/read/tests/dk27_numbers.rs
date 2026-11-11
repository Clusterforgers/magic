//! Goal: Wheel of Misfortune's secret numbers read as a `ChooseNumbers` and the damage, discards and draws that read its
//! names, write back as the printed text, and a near miss of the text or of the rules reads or writes as something else.
use super::*;

const WHEEL: &str = "Each player secretly chooses a number 0 or greater, then all players reveal those numbers simultaneously and determine the highest and lowest numbers revealed this way. ~ deals damage equal to the highest number to each player who chose that number. Each player who didn't choose the lowest number discards their hand, then draws seven cards.";

const NOT_LOWEST: &str =
    r#"{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"Among":{"Bound":"lowest"}}}}}"#;

fn rules() -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"ChooseNumbers":{{"secret":true,"as":"number","highest":"highest","lowest":"lowest"}}}},{{"DealDamage":{{"to":{{"Bound":"highest"}},"amount":{{"ChosenAs":"number"}}}}}},{{"Discard":{{"who":{NOT_LOWEST},"count":{{"Count":[{{"In":"Hand"}},{{"OwnedBy":"EachPlayer"}}]}}}}}},{{"Draw":{{"who":{NOT_LOWEST},"count":7}}}}]}}}}"#
    )
}

#[test]
fn wheel_of_misfortune_reads_and_writes_back() {
    reads_as(WHEEL, ClauseKind::Spell, false, &rules());
}

#[rstest]
#[case::open("Each player secretly", "Each player")]
#[case::one_or_greater("number 0 or greater", "number 1 or greater")]
#[case::lowest_damaged("the highest number to each", "the lowest number to each")]
#[case::who_chose_the_lowest("who didn't choose the lowest", "who chose the lowest")]
#[case::six_cards("draws seven cards", "draws six cards")]
fn a_near_miss_of_the_text_stays_unread(#[case] from: &str, #[case] to: &str) {
    let text = WHEEL.replace(from, to);
    assert_eq!(clause(&text, ClauseKind::Spell, false), None, "{text}");
}

#[rstest]
#[case::open(r#""secret":true,"#, "")]
#[case::from_one(r#""secret":true,"#, r#""secret":true,"min":1,"#)]
#[case::the_lowest_damaged(r#""to":{"Bound":"highest"}"#, r#""to":{"Bound":"lowest"}"#)]
#[case::six_cards(r#""count":7"#, r#""count":6"#)]
#[case::only_opponents(r#""ChooseNumbers":{"#, r#""ChooseNumbers":{"who":"Opponents","#)]
fn a_near_miss_of_the_rules_is_not_written_as_the_text(#[case] from: &str, #[case] to: &str) {
    let json = rules().replacen(from, to, 1);
    let written = render_variants(&ability(&json));
    assert!(
        !written.iter().any(|said| same_text(said, WHEEL)),
        "{json}: {written:?}"
    );
}
