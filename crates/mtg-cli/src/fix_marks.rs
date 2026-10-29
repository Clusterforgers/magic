//! `fix-marks`: the cards players marked as important to fix, ranked by how many accounts marked them, with the support
//! state the game gives each card now and the newest note (`docs/server.md`, Fix marks).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use mtg_cards::load::GeneratedRules;
use mtg_cards::support::Support;
use mtg_cards::{builtin, load::Library};
use mtg_db::{Database, RankedMark};
use mtg_import::ids::OracleId;

use crate::args::FixMarksArgs;
use crate::coverage::printed_card;
use crate::error::CliError;
use crate::status::missing_table;

/// The longest card name of the table; a longer one is cut and ends in `...`.
const NAME_WIDTH: usize = 40;

/// The longest note of the table; a longer one is cut and ends in `...`.
const NOTE_WIDTH: usize = 80;

/// One row of the table: a marked card as it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The card's name.
    pub name: String,
    /// How many accounts marked it.
    pub marks: u32,
    /// How far the game plays it now; `None` when the card could not be read.
    pub support: Option<Support>,
    /// The note of the newest mark that has one.
    pub note: Option<String>,
}

/// The word of a support state.
pub const fn support_word(support: Support) -> &'static str {
    match support {
        Support::Verified => "verified",
        Support::Partial => "partial",
        Support::Manual => "manual",
        Support::Stale => "stale",
        Support::Unplayable => "unplayable",
    }
}

/// `text` cut to `width` characters, ending in `...` when it was longer.
fn cut(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let kept: String = text.chars().take(width.saturating_sub(3)).collect();
    format!("{kept}...")
}

/// A note on one line: line breaks and tabs are spaces.
fn one_line(note: &str) -> String {
    note.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The table: a header counting the cards, then a row for each with its rank, marks, support state, name and note.
pub fn table(rows: &[Row], total: usize) -> String {
    if rows.is_empty() {
        return "no card is marked as important to fix yet".to_owned();
    }
    let name_width = rows
        .iter()
        .map(|row| row.name.chars().count().min(NAME_WIDTH))
        .max()
        .unwrap_or(0)
        .max("card".len());
    let mut text = format!(
        "fix marks (top {} of {} marked cards; marks: accounts that marked the card)\n",
        rows.len(),
        total
    );
    let _ = writeln!(
        text,
        "{:>3} {:>5}  {:<10}  {:<name_width$}  note",
        "#", "marks", "support", "card"
    );
    for (i, row) in rows.iter().enumerate() {
        let note = row.note.as_deref().map(one_line).unwrap_or_default();
        let line = format!(
            "{:>3} {:>5}  {:<10}  {:<name_width$}  {}",
            i + 1,
            row.marks,
            row.support.map_or("unknown", support_word),
            cut(&row.name, NAME_WIDTH),
            cut(&note, NOTE_WIDTH),
        );
        let _ = writeln!(text, "{}", line.trim_end());
    }
    text.trim_end().to_owned()
}

/// The library the server reads cards with: the hand-written rules and the compiled rows of the database.
async fn library(db: &Database) -> Result<Library, CliError> {
    let hand_written = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    let rows: Vec<GeneratedRules> = db
        .card_rules()
        .all()
        .await?
        .into_iter()
        .map(|row| GeneratedRules {
            oracle_id: row.oracle_id.as_str().to_owned(),
            rules: row.rules,
        })
        .collect();
    Ok(hand_written.with_generated(&rows).0)
}

/// The support state the game gives each marked card now, by oracle id.
async fn supports(
    db: &Database,
    ranked: &[RankedMark],
) -> Result<BTreeMap<String, Support>, CliError> {
    let ids: Vec<OracleId> = ranked.iter().map(|mark| mark.oracle_id.clone()).collect();
    let cards = db.cards().get_many(&ids).await?;
    let library = library(db).await?;
    let caps = mtg_engine::caps();
    Ok(cards
        .iter()
        .map(|card| {
            let printed = printed_card(card);
            let support = library.support(&printed, &caps).support;
            (card.oracle_id.as_str().to_owned(), support)
        })
        .collect())
}

/// The marked cards joined with their support state, in the order of the ranking.
pub fn rows(ranked: &[RankedMark], supports: &BTreeMap<String, Support>) -> Vec<Row> {
    ranked
        .iter()
        .map(|mark| Row {
            name: mark.name.clone(),
            marks: mark.marks,
            support: supports.get(mark.oracle_id.as_str()).copied(),
            note: mark.newest_note.clone(),
        })
        .collect()
}

/// The text of `fix-marks`: the table of the most marked cards.
pub async fn fix_marks(db: &Database, args: &FixMarksArgs) -> Result<String, CliError> {
    let repo = db.fix_marks();
    let wrong_schema = |error: mtg_db::DbError| {
        if missing_table(&error) {
            CliError::Config("the schema is not applied yet; `mtg-cli ingest` applies it".into())
        } else {
            CliError::Db(error)
        }
    };
    let all = repo.ranked(None).await.map_err(wrong_schema)?;
    let total = all.len();
    let top: Vec<RankedMark> = all.into_iter().take(args.limit as usize).collect();
    let supports = supports(db, &top).await?;
    Ok(table(&rows(&top, &supports), total))
}

#[cfg(test)]
mod tests {
    //! Goal: the table ranks as given, shows support state and the one-line note, cuts long names and notes, pads to the longest name, says so when nothing is marked, and joins the support state by oracle id.
    use super::*;

    fn row(name: &str, marks: u32, support: Option<Support>, note: Option<&str>) -> Row {
        Row {
            name: name.to_owned(),
            marks,
            support,
            note: note.map(str::to_owned),
        }
    }

    #[test]
    fn the_table_lists_rank_marks_support_card_and_note() {
        let text = table(
            &[
                row(
                    "Sol Ring",
                    12,
                    Some(Support::Partial),
                    Some("adds the wrong\nmana"),
                ),
                row("Opt", 2, Some(Support::Manual), None),
                row("Gone", 1, None, None),
            ],
            9,
        );
        let expected = [
            "fix marks (top 3 of 9 marked cards; marks: accounts that marked the card)",
            "  #  marks  support     card      note",
            "  1     12  partial     Sol Ring  adds the wrong mana",
            "  2      2  manual      Opt",
            "  3      1  unknown     Gone",
        ];
        assert_eq!(text, expected.join("\n"));
    }

    #[test]
    fn a_long_name_and_a_long_note_are_cut() {
        let name = "n".repeat(NAME_WIDTH + 10);
        let note = "w ".repeat(NOTE_WIDTH);
        let text = table(&[row(&name, 1, Some(Support::Stale), Some(&note))], 1);
        let line = text.lines().nth(2).unwrap();
        assert!(
            line.contains(&format!("{}...", "n".repeat(NAME_WIDTH - 3))),
            "{line}"
        );
        assert!(line.ends_with("..."), "{line}");
        assert_eq!(
            line.chars().count(),
            3 + 1 + 5 + 2 + 10 + 2 + NAME_WIDTH + 2 + NOTE_WIDTH
        );
    }

    #[test]
    fn nothing_marked_is_said_in_one_line() {
        assert_eq!(table(&[], 0), "no card is marked as important to fix yet");
    }

    #[test]
    fn the_support_state_is_joined_by_oracle_id() {
        let ranked = vec![
            RankedMark {
                oracle_id: OracleId("a".into()),
                name: "A".into(),
                marks: 3,
                newest_note: Some("x".into()),
            },
            RankedMark {
                oracle_id: OracleId("b".into()),
                name: "B".into(),
                marks: 1,
                newest_note: None,
            },
        ];
        let known = BTreeMap::from([("a".to_owned(), Support::Unplayable)]);
        assert_eq!(
            rows(&ranked, &known),
            [
                row("A", 3, Some(Support::Unplayable), Some("x")),
                row("B", 1, None, None),
            ]
        );
    }
}
