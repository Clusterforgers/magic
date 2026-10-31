//! `reads`: the verdicts players gave the drafts of the read tier, newest first, and the library `coverage` judges with
//! the stored drafts and their read state (`docs/server.md`, Reads).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use chrono::{DateTime, Duration, Utc};
use mtg_cards::load::Library;
use mtg_cards::{DraftRules, ReadState as CardsReadState, builtin};
use mtg_db::{Database, DbError, DraftRead, ReadState, StoredRead, Verdict};
use mtg_import::ids::OracleId;
use sqlx::types::uuid::Uuid;

use crate::args::ReadsArgs;
use crate::error::CliError;
use crate::status::missing_table;

/// Two verdicts of one account on different cards that come less than this far apart are flagged by `--quick`.
pub const QUICK_GAP: Duration = Duration::seconds(4);

/// The longest card name of the table; a longer one is cut and ends in `...`.
const NAME_WIDTH: usize = 36;

/// The longest note of the table; a longer one is cut and ends in `...`.
const NOTE_WIDTH: usize = 60;

/// The word of an account in the table: its id, or `server` for the verdict of a resolution that failed in a game.
fn account_word(user: Option<Uuid>) -> String {
    user.map_or_else(|| "server".to_owned(), |user| user.to_string())
}

/// The word of a verdict.
const fn verdict_word(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Correct => "correct",
        Verdict::Wrong => "wrong",
    }
}

/// The word of a read state.
const fn state_word(state: ReadState) -> &'static str {
    match state {
        ReadState::Pending => "pending",
        ReadState::Approved => "approved",
        ReadState::Wrong => "wrong",
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

/// The ids of the verdicts that an account gave on different cards less than [`QUICK_GAP`] from another verdict of its
/// own. The server's verdicts have no account and never count; a second verdict on the same card is a change of mind.
pub fn quick_ids(reads: &[StoredRead]) -> BTreeSet<i64> {
    let mut by_account: BTreeMap<Uuid, Vec<&StoredRead>> = BTreeMap::new();
    for read in reads {
        if let Some(user) = read.user_id {
            by_account.entry(user).or_default().push(read);
        }
    }
    let mut flagged = BTreeSet::new();
    for given in by_account.values_mut() {
        given.sort_by_key(|read| (read.created_at, read.id));
        for (i, read) in given.iter().enumerate() {
            for earlier in given[..i]
                .iter()
                .rev()
                .take_while(|earlier| read.created_at - earlier.created_at < QUICK_GAP)
            {
                if earlier.oracle_id != read.oracle_id {
                    flagged.extend([read.id, earlier.id]);
                }
            }
        }
    }
    flagged
}

/// One row of the table: a verdict as it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// When the verdict was given.
    pub at: DateTime<Utc>,
    /// The card's name.
    pub name: String,
    /// The read state of the card's current draft when the verdict is on its hash; `None` for a verdict on a replaced one.
    pub state: Option<ReadState>,
    /// The account, or none for the server.
    pub account: Option<Uuid>,
    /// What the verdict says.
    pub verdict: Verdict,
    /// Why.
    pub note: Option<String>,
}

/// The verdicts newest first, each with the state of its draft when that is still the card's current one.
pub fn rows(reads: &[StoredRead], states: &[DraftRead]) -> Vec<Row> {
    let current: BTreeMap<&OracleId, &DraftRead> = states
        .iter()
        .map(|state| (&state.oracle_id, state))
        .collect();
    let mut ordered: Vec<&StoredRead> = reads.iter().collect();
    ordered.sort_by_key(|read| std::cmp::Reverse((read.created_at, read.id)));
    ordered
        .into_iter()
        .map(|read| Row {
            at: read.created_at,
            name: read.name.clone(),
            state: current
                .get(&read.oracle_id)
                .filter(|draft| draft.read_hash == read.read_hash)
                .map(|draft| draft.state),
            account: read.user_id,
            verdict: read.verdict,
            note: read.note.clone(),
        })
        .collect()
}

/// The table: a header counting the verdicts, then a row for each with its time, card, read state, account, verdict and note.
pub fn table(rows: &[Row], total: usize, quick: bool) -> String {
    let name_width = rows
        .iter()
        .map(|row| row.name.chars().count().min(NAME_WIDTH))
        .max()
        .unwrap_or(0)
        .max("card".len());
    let what = if quick { "quick verdicts" } else { "verdicts" };
    let mut text = format!(
        "reads (newest {} of {total} {what}; read: the state of the card's draft, old for a replaced one)\n",
        rows.len()
    );
    let _ = writeln!(
        text,
        "{:<19}  {:<name_width$}  {:<8}  {:<36}  {:<7}  note",
        "time (UTC)", "card", "read", "account", "verdict"
    );
    for row in rows {
        let note = row.note.as_deref().map(one_line).unwrap_or_default();
        let line = format!(
            "{:<19}  {:<name_width$}  {:<8}  {:<36}  {:<7}  {}",
            row.at.format("%Y-%m-%d %H:%M:%S"),
            cut(&row.name, NAME_WIDTH),
            row.state.map_or("old", state_word),
            account_word(row.account),
            verdict_word(row.verdict),
            cut(&note, NOTE_WIDTH),
        );
        let _ = writeln!(text, "{}", line.trim_end());
    }
    text.trim_end().to_owned()
}

/// The accounts that have quick verdicts, with how many of their verdicts are quick and how many they gave, the most
/// quick verdicts first.
pub fn quick_accounts(reads: &[StoredRead], quick: &BTreeSet<i64>) -> String {
    let mut counts: BTreeMap<Uuid, (usize, usize)> = BTreeMap::new();
    for read in reads {
        let Some(user) = read.user_id else { continue };
        let entry = counts.entry(user).or_default();
        entry.1 += 1;
        entry.0 += usize::from(quick.contains(&read.id));
    }
    let mut accounts: Vec<(Uuid, (usize, usize))> =
        counts.into_iter().filter(|(_, (n, _))| *n > 0).collect();
    accounts.sort_by_key(|&(user, (quick, all))| (std::cmp::Reverse(quick), user, all));
    let mut text = format!(
        "{} gave verdicts on different cards less than {} s apart\n{:<36}  {:>5}  {:>5}\n",
        if accounts.len() == 1 {
            "1 account".to_owned()
        } else {
            format!("{} accounts", accounts.len())
        },
        QUICK_GAP.num_seconds(),
        "account",
        "quick",
        "all"
    );
    for (user, (quick, all)) in accounts {
        let _ = writeln!(text, "{user:<36}  {quick:>5}  {all:>5}");
    }
    text
}

/// The text of `reads`: the verdicts, newest first.
pub async fn reads(db: &Database, args: &ReadsArgs) -> Result<String, CliError> {
    let schema = |error: DbError| {
        if missing_table(&error) {
            CliError::Config("the schema is not applied yet; `mtg-cli ingest` applies it".into())
        } else {
            CliError::Db(error)
        }
    };
    // A quick verdict can be a `correct` one beside a `wrong` one, so `--quick` always reads them all.
    let stored = db
        .reads()
        .list(args.wrong && !args.quick)
        .await
        .map_err(schema)?;
    let states = db.reads().state().await.map_err(schema)?;
    let quick = args.quick.then(|| quick_ids(&stored));
    let shown: Vec<StoredRead> = stored
        .iter()
        .filter(|read| quick.as_ref().is_none_or(|quick| quick.contains(&read.id)))
        .filter(|read| !args.wrong || read.verdict == Verdict::Wrong)
        .cloned()
        .collect();
    if shown.is_empty() {
        return Ok(match (args.quick, args.wrong) {
            (true, _) => "no account gave verdicts on different cards less than 4 seconds apart",
            (false, true) => "no wrong verdict is stored yet",
            (false, false) => "no read verdict is stored yet",
        }
        .to_owned());
    }
    let total = shown.len();
    let listed: Vec<Row> = rows(&shown, &states)
        .into_iter()
        .take(args.limit as usize)
        .collect();
    let table = table(&listed, total, args.quick);
    Ok(match &quick {
        Some(quick) => format!("{}\n{table}", quick_accounts(&stored, quick)),
        None => table,
    })
}

/// The read state of the cards tier as the library takes it.
const fn cards_state(state: ReadState) -> CardsReadState {
    match state {
        ReadState::Pending => CardsReadState::Pending,
        ReadState::Approved => CardsReadState::Approved,
        ReadState::Wrong => CardsReadState::Wrong,
    }
}

/// The library `coverage` judges with: the embedded rules files and the stored current drafts with the read state their
/// verdicts come to, and the count of stored drafts the library had to leave out (they do not load).
pub(crate) async fn draft_library(db: &Database) -> Result<(Library, usize), CliError> {
    let hand_written = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    let schema = |error: DbError| {
        if missing_table(&error) {
            CliError::Config("the schema is not applied yet; `mtg-cli ingest` applies it".into())
        } else {
            CliError::Db(error)
        }
    };
    let drafts = db.drafts().current().await.map_err(schema)?;
    let states: BTreeMap<OracleId, DraftRead> = db
        .reads()
        .state()
        .await
        .map_err(schema)?
        .into_iter()
        .map(|state| (state.oracle_id.clone(), state))
        .collect();
    let rows: Vec<DraftRules> = drafts
        .into_iter()
        .map(|draft| {
            let read = states
                .get(&draft.oracle_id)
                .filter(|state| state.read_hash == draft.read_hash)
                .map_or(CardsReadState::Pending, |state| cards_state(state.state));
            DraftRules {
                oracle_id: draft.oracle_id.as_str().to_owned(),
                read_hash: draft.read_hash,
                rules: draft.rules,
                read,
            }
        })
        .collect();
    let (library, left_out) = hand_written.with_drafts(&rows);
    let skipped = left_out
        .iter()
        .map(|error| error.file.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    Ok((library, skipped))
}

#[cfg(test)]
mod tests {
    //! Goal: `--quick` flags an account's verdicts on different cards less than four seconds apart and nothing else (not exactly four seconds, not the same card, not another account, not the server), the rows come newest first with the state of the draft they are on, and the table cuts long names and notes and pads to the longest name.
    use chrono::TimeZone;
    use rstest::rstest;

    use super::*;

    fn at(millis: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(1_700_000_000_000 + millis)
            .single()
            .unwrap_or_default()
    }

    fn read(id: i64, by: u128, card: &str, millis: i64, verdict: Verdict) -> StoredRead {
        StoredRead {
            id,
            user_id: (by != 0).then(|| Uuid::from_u128(by)),
            oracle_id: OracleId(card.to_owned()),
            name: card.to_uppercase(),
            read_hash: format!("hash-{card}"),
            verdict,
            note: (verdict == Verdict::Wrong).then(|| "no".to_owned()),
            created_at: at(millis),
        }
    }

    /// Verdicts as (id, account, card, milliseconds after the first) and the ids `--quick` flags.
    #[rstest]
    #[case::three_seconds_apart(&[(1, 1, "a", 0), (2, 1, "b", 3_000)], &[1, 2])]
    #[case::just_under_four_seconds(&[(1, 1, "a", 0), (2, 1, "b", 3_999)], &[1, 2])]
    #[case::exactly_four_seconds(&[(1, 1, "a", 0), (2, 1, "b", 4_000)], &[])]
    #[case::the_same_instant(&[(1, 1, "a", 0), (2, 1, "b", 0)], &[1, 2])]
    #[case::the_same_card_twice_is_a_change_of_mind(&[(1, 1, "a", 0), (2, 1, "a", 500)], &[])]
    #[case::another_account(&[(1, 1, "a", 0), (2, 2, "b", 500)], &[])]
    #[case::the_server(&[(1, 0, "a", 0), (2, 0, "b", 500)], &[])]
    #[case::a_chain_flags_each_link(
        &[(1, 1, "a", 0), (2, 1, "b", 3_500), (3, 1, "c", 7_000), (4, 1, "d", 20_000)],
        &[1, 2, 3]
    )]
    #[case::a_second_verdict_on_a_card_does_not_hide_the_one_before_it(
        &[(1, 1, "a", 0), (2, 1, "a", 100), (3, 1, "b", 200)],
        &[1, 2, 3]
    )]
    #[case::input_order_does_not_matter(&[(2, 1, "b", 3_000), (1, 1, "a", 0)], &[1, 2])]
    #[case::two_accounts_are_judged_apart(
        &[(1, 1, "a", 0), (2, 2, "b", 1_000), (3, 1, "c", 2_000), (4, 2, "d", 30_000)],
        &[1, 3]
    )]
    fn quick_flags_one_accounts_verdicts_on_different_cards_less_than_four_seconds_apart(
        #[case] given: &[(i64, u128, &str, i64)],
        #[case] flagged: &[i64],
    ) {
        let reads: Vec<StoredRead> = given
            .iter()
            .map(|&(id, by, card, millis)| read(id, by, card, millis, Verdict::Correct))
            .collect();
        assert_eq!(quick_ids(&reads).into_iter().collect::<Vec<_>>(), flagged);
    }

    fn state(card: &str, state: ReadState) -> DraftRead {
        DraftRead {
            oracle_id: OracleId(card.to_owned()),
            read_hash: format!("hash-{card}"),
            state,
            correct: 0,
            wrong: 0,
        }
    }

    #[test]
    fn rows_come_newest_first_with_the_state_of_the_draft_they_are_on() {
        let mut replaced = read(3, 2, "c", 3_000, Verdict::Correct);
        replaced.read_hash = "an older hash".to_owned();
        let reads = [
            read(1, 1, "a", 1_000, Verdict::Wrong),
            read(2, 0, "b", 2_000, Verdict::Wrong),
            replaced,
            read(4, 1, "a", 3_000, Verdict::Correct),
        ];
        let states = [
            state("a", ReadState::Wrong),
            state("b", ReadState::Wrong),
            state("c", ReadState::Approved),
        ];

        let rows = rows(&reads, &states);

        let seen: Vec<(&str, Option<ReadState>, Option<Uuid>)> = rows
            .iter()
            .map(|row| (row.name.as_str(), row.state, row.account))
            .collect();
        assert_eq!(
            seen,
            [
                ("A", Some(ReadState::Wrong), Some(Uuid::from_u128(1))),
                ("C", None, Some(Uuid::from_u128(2))),
                ("B", Some(ReadState::Wrong), None),
                ("A", Some(ReadState::Wrong), Some(Uuid::from_u128(1))),
            ]
        );
    }

    fn row(name: &str, state: Option<ReadState>, by: Option<Uuid>, note: Option<&str>) -> Row {
        Row {
            at: at(0),
            name: name.to_owned(),
            state,
            account: by,
            verdict: if note.is_some() {
                Verdict::Wrong
            } else {
                Verdict::Correct
            },
            note: note.map(str::to_owned),
        }
    }

    #[test]
    fn the_table_lists_time_card_state_account_verdict_and_the_one_line_note() {
        let text = table(
            &[
                row(
                    "Sol Ring",
                    Some(ReadState::Wrong),
                    Some(Uuid::from_u128(7)),
                    Some("adds\nthe wrong mana"),
                ),
                row("Opt", Some(ReadState::Approved), None, None),
                row("Gone", None, Some(Uuid::from_u128(8)), None),
            ],
            9,
            false,
        );
        let user = |n: u128| Uuid::from_u128(n).to_string();
        let expected = [
            "reads (newest 3 of 9 verdicts; read: the state of the card's draft, old for a replaced one)".to_owned(),
            format!("time (UTC)           card      read      {:<36}  verdict  note", "account"),
            format!("2023-11-14 22:13:20  Sol Ring  wrong     {}  wrong    adds the wrong mana", user(7)),
            format!("2023-11-14 22:13:20  Opt       approved  {:<36}  correct", "server"),
            format!("2023-11-14 22:13:20  Gone      old       {}  correct", user(8)),
        ];
        assert_eq!(text, expected.join("\n"));
    }

    #[test]
    fn a_long_name_and_a_long_note_are_cut() {
        let name = "n".repeat(NAME_WIDTH + 10);
        let note = "w ".repeat(NOTE_WIDTH);
        let text = table(&[row(&name, None, None, Some(&note))], 1, true);
        let line = text.lines().nth(2).unwrap_or_default();
        assert!(
            text.starts_with("reads (newest 1 of 1 quick verdicts;"),
            "{text}"
        );
        assert!(
            line.contains(&format!("{}...", "n".repeat(NAME_WIDTH - 3))),
            "{line}"
        );
        assert!(line.ends_with("..."), "{line}");
        assert_eq!(
            line.chars().count(),
            19 + 2 + NAME_WIDTH + 2 + 8 + 2 + 36 + 2 + 7 + 2 + NOTE_WIDTH
        );
    }

    #[test]
    fn the_account_summary_counts_quick_and_all_verdicts_the_quickest_first() {
        let reads = [
            read(1, 1, "a", 0, Verdict::Correct),
            read(2, 1, "b", 1_000, Verdict::Correct),
            read(3, 1, "c", 60_000, Verdict::Correct),
            read(4, 2, "a", 0, Verdict::Correct),
            read(5, 2, "b", 1_000, Verdict::Correct),
            read(6, 2, "c", 2_000, Verdict::Correct),
            read(7, 3, "a", 0, Verdict::Correct),
        ];
        let text = quick_accounts(&reads, &quick_ids(&reads));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "2 accounts gave verdicts on different cards less than 4 s apart"
        );
        assert!(
            lines[2].starts_with(&Uuid::from_u128(2).to_string()),
            "{text}"
        );
        assert!(lines[2].ends_with("    3      3"), "{text}");
        assert!(lines[3].ends_with("    2      3"), "{text}");
        assert_eq!(lines.len(), 4, "{text}");
    }
}
