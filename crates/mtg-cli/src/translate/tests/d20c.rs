//! Goal: `Proliferate` with `Amount$` runs that many times, and a pump of a player that gives protection from everything is a rule for the duration, while any other player keyword stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn artifact(line: &str) -> String {
    format!("Name:Subject\nManaCost:2\nTypes:Artifact\n{line}\n")
}

#[rstest]
#[case::twice("2", 2)]
#[case::thrice("3", 3)]
fn proliferate_amount_repeats_the_step(#[case] amount: &str, #[case] times: usize) {
    let found = abilities(&artifact(&format!(
        "A:AB$ Proliferate | Cost$ T | Amount$ {amount}"
    )));
    let effects = found[0]["Activated"]["effects"].as_array().cloned();
    assert_eq!(effects, Some(vec![json!({"Proliferate": {}}); times]));
}

#[rstest]
#[case::not_a_number("X")]
#[case::zero("0")]
fn proliferate_amount_stays_a_gap_when_unreadable(#[case] amount: &str) {
    let script = artifact(&format!("A:AB$ Proliferate | Cost$ T | Amount$ {amount}"));
    assert_eq!(gaps(&script), vec!["key:Proliferate.Amount".to_owned()]);
}

#[rstest]
#[case::everything("Protection from everything", true)]
#[case::from_red("Protection from red", false)]
fn player_protection_is_a_rule_only_for_everything(#[case] keyword: &str, #[case] mapped: bool) {
    let script = artifact(&format!(
        "A:AB$ Pump | Cost$ T | Defined$ You | Duration$ UntilYourNextTurn | KW$ {keyword}"
    ));
    if mapped {
        let rule = &abilities(&script)[0]["Activated"]["effects"][0]["Rule"];
        assert_eq!(
            rule["rule"],
            json!({"PlayersHave": {"who": "You", "keyword": {"Protection": []}}})
        );
        assert_eq!(rule["duration"], json!("UntilYourNextTurn"));
    } else {
        assert_eq!(
            gaps(&script),
            vec!["key:Pump.player-keyword-Protection from red".to_owned()]
        );
    }
}
