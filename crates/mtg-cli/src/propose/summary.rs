//! What became of each card of a run, one record a card, and the summary TSV the engine packs read: the outcome of every
//! card, the engine pieces the model says are missing, what it cost.

use serde::{Deserialize, Serialize};

use super::price::Usd;

/// The first line of the summary.
pub const SUMMARY_HEADER: &str =
    "card\toutcome\tmissing_engine_piece\trequests\tusd\tdetail\tagree";

/// How a card ended, as the word of the `outcome` column, in the order the summary lists them.
pub const OUTCOMES: [&str; 8] = [
    "verified",
    "partial",
    "draft",
    "mismatch",
    "invalid",
    "empty",
    "no_answer",
    "not_run",
];

/// The longest detail a summary line keeps.
const DETAIL_LENGTH: usize = 300;

/// One card's result: a draft the checker accepts (or an accepted proposal) or a structured failure with the engine
/// pieces that are missing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardRecord {
    /// The card's name.
    pub card: String,
    /// `verified` and `partial` (accepted proposals), `draft` (valid, nothing contradicts the text, a construct with no
    /// writer), or a failure: `mismatch`, `invalid`, `empty`, `no_answer`.
    pub outcome: String,
    /// What the model says the card needs that the DSL cannot say; empty when it said nothing.
    pub missing_engine_piece: Vec<String>,
    /// Requests made for the card, the repair round included.
    pub requests: u32,
    /// What they cost in micro-dollars (zero for a local model).
    pub micros: u64,
    /// The first thing the checker found wrong, the constructs without a writer, or the API's complaint.
    pub detail: String,
    /// With several sampled answers (`propose --lines --samples N`): whether they all said the same; none otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agree: Option<bool>,
}

impl CardRecord {
    /// The record as one summary line.
    pub fn line(&self) -> String {
        let one_line = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
        let detail: String = one_line(&self.detail).chars().take(DETAIL_LENGTH).collect();
        let agree = match self.agree {
            Some(true) => "yes",
            Some(false) => "no",
            None => "",
        };
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{agree}",
            one_line(&self.card).replace('\t', " "),
            self.outcome,
            one_line(&self.missing_engine_piece.join(" | ")).replace('\t', " "),
            self.requests,
            Usd(self.micros).plain(),
            detail.replace('\t', " ")
        )
    }
}

/// The summary of `records`: the header, then the cards by outcome in the order of [`OUTCOMES`] and by name.
pub fn render_summary(records: &[CardRecord]) -> String {
    let rank = |record: &CardRecord| {
        OUTCOMES
            .iter()
            .position(|word| *word == record.outcome)
            .unwrap_or(OUTCOMES.len())
    };
    let mut sorted: Vec<&CardRecord> = records.iter().collect();
    sorted.sort_by(|a, b| (rank(a), &a.card).cmp(&(rank(b), &b.card)));
    let mut text = format!("{SUMMARY_HEADER}\n");
    for record in sorted {
        text.push_str(&record.line());
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    //! Goal: the summary is a TSV with one line a card, the cards by outcome and name, the missing engine pieces in one
    //! column, tabs and line breaks never splitting a column, and a record survives its JSON.
    use super::*;

    fn record(card: &str, outcome: &str, missing: &[&str]) -> CardRecord {
        CardRecord {
            card: card.into(),
            outcome: outcome.into(),
            missing_engine_piece: missing.iter().map(|piece| (*piece).to_owned()).collect(),
            requests: 2,
            micros: 12_345,
            detail: "ability 0 reads back as \"x\"\nbut the card says \"y\"".into(),
            agree: None,
        }
    }

    #[test]
    fn lines_are_sorted_by_outcome_then_name_under_one_header() {
        let text = render_summary(&[
            record("Zed", "invalid", &[]),
            record("Opt", "verified", &[]),
            record("Abe", "invalid", &["replacement: exile instead"]),
            record("Mid", "draft", &[]),
        ]);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], SUMMARY_HEADER);
        let names: Vec<&str> = lines[1..]
            .iter()
            .map(|line| line.split('\t').next().unwrap())
            .collect();
        assert_eq!(names, ["Opt", "Mid", "Abe", "Zed"]);
    }

    #[test]
    fn a_line_has_seven_columns_whatever_the_text_holds() {
        let mut odd = record("Odd\tName", "mismatch", &["a\tb", "c\nd"]);
        odd.detail = "x\ty\n".repeat(200);
        let line = odd.line();
        let columns: Vec<&str> = line.split('\t').collect();
        assert_eq!(columns.len(), 7, "{line}");
        assert_eq!(columns[0], "Odd Name");
        assert_eq!(columns[2], "a b | c d");
        assert_eq!(columns[3..5], ["2", "0.012345"]);
        assert!(columns[5].chars().count() <= DETAIL_LENGTH);
        assert_eq!(columns[6], "");
        assert!(!line.contains('\n'));
    }

    #[test]
    fn sampled_answers_say_whether_they_agree() {
        let mut agreed = record("Opt", "verified", &[]);
        agreed.agree = Some(true);
        assert!(agreed.line().ends_with("\tyes"));
        agreed.agree = Some(false);
        assert!(agreed.line().ends_with("\tno"));
        let json = serde_json::to_string(&agreed).unwrap();
        assert_eq!(serde_json::from_str::<CardRecord>(&json).unwrap(), agreed);
    }

    #[test]
    fn a_record_survives_json() {
        let original = record("Opt", "draft", &["a", "b"]);
        let json = serde_json::to_string(&original).unwrap();
        assert!(
            json.contains("\"missing_engine_piece\":[\"a\",\"b\"]"),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<CardRecord>(&json).unwrap(), original);
    }
}
