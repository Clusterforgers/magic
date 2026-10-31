//! `coverage --deck`: the cards of the selected precons that are neither verified nor read, in popularity order, with the
//! first blocker of each: what still needs a draft (or, for a waiting card, a read).

use std::fmt::Write as _;

use mtg_cards::coverage::Report;
use mtg_cards::support::Support;

use crate::coverage_all::entries;
use crate::popularity::Popularity;

/// The longest card name of the list; a longer one is cut and ends in `...`.
const NAME_WIDTH: usize = 40;

/// The longest blocker of the list; a longer one is cut and ends in `...`.
const BLOCKER_WIDTH: usize = 100;

/// `text` cut to `width` characters, ending in `...` when it was longer.
fn cut(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let kept: String = text.chars().take(width.saturating_sub(3)).collect();
    format!("{kept}...")
}

/// The list: a header counting the cards, then a row for each card that is neither `Verified` nor `Read` with its rank
/// (`-` without one), state (`waiting` when a draft of it awaits a read), name and first blocker, the most played first
/// and the cards without a rank by name. The first blocker is the first of the card's blockers in report order.
pub fn render(report: &Report, popularity: &Popularity) -> String {
    let entries = entries(report, popularity);
    let open: Vec<_> = entries
        .iter()
        .filter(|entry| !matches!(entry.result.support, Support::Verified | Support::Read))
        .collect();
    if open.is_empty() {
        return format!(
            "cards to draft: none, all {} distinct cards are verified or read",
            entries.len()
        );
    }
    let name_width = open
        .iter()
        .map(|entry| entry.result.name.chars().count().min(NAME_WIDTH))
        .max()
        .unwrap_or(0)
        .max("card".len());
    let mut text = format!(
        "cards to draft: {} of {} distinct cards are neither verified nor read, most played first (waiting: a draft awaits its first read)\n",
        open.len(),
        entries.len()
    );
    let _ = writeln!(
        text,
        "{:>6}  {:<10}  {:<name_width$}  first blocker",
        "rank", "state", "card"
    );
    for entry in open {
        let result = entry.result;
        let rank = entry
            .rank
            .map_or_else(|| "-".to_owned(), |rank| rank.to_string());
        let blocker = result
            .blockers
            .iter()
            .next()
            .map(|blocker| cut(&blocker.label(), BLOCKER_WIDTH))
            .unwrap_or_default();
        let line = format!(
            "{rank:>6}  {:<10}  {:<name_width$}  {blocker}",
            result.bucket().name(),
            cut(&result.name, NAME_WIDTH),
        );
        let _ = writeln!(text, "{}", line.trim_end());
    }
    text.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    //! Goal: the list holds the cards that are neither verified nor read in popularity order with unranked cards last by name, names the waiting ones, shows the first blocker, cuts long names and blockers, and says so when nothing is left.
    use std::collections::{BTreeMap, BTreeSet};

    use mtg_cards::coverage::{Blocker, CardResult, RulesSource};

    use super::*;

    fn result(id: &str, support: Support, waiting: bool, blockers: &[&str]) -> CardResult {
        CardResult {
            oracle_id: id.to_owned(),
            name: format!("Card {id}"),
            rules: RulesSource::Compiled,
            support,
            blockers: blockers
                .iter()
                .map(|shape| Blocker::Clause((*shape).to_owned()))
                .collect::<BTreeSet<_>>(),
            waiting,
            copies: BTreeMap::new(),
        }
    }

    /// Card `a` has rank 1, `b` 2, `c` 3 and the others none.
    fn popularity() -> Popularity {
        let ranks = [("a", 1), ("b", 2), ("c", 3)];
        Popularity::new(
            ranks
                .into_iter()
                .map(|(id, n)| (id.to_owned(), n))
                .collect(),
        )
        .unwrap_or_else(|| unreachable!("the ranks are not empty"))
    }

    #[test]
    fn open_cards_come_most_played_first_with_their_state_and_first_blocker() {
        let report = Report {
            cards: vec![
                result("a", Support::Verified, false, &[]),
                result("b", Support::Read, false, &[]),
                result("c", Support::Manual, true, &["y", "x"]),
                result("d", Support::Partial, false, &["z"]),
                result("e", Support::Stale, false, &[]),
            ],
        };
        let expected = [
            "cards to draft: 3 of 5 distinct cards are neither verified nor read, most played first (waiting: a draft awaits its first read)",
            "  rank  state       card    first blocker",
            "     3  waiting     Card c  x",
            "     -  partial     Card d  z",
            "     -  other       Card e",
        ];
        assert_eq!(render(&report, &popularity()), expected.join("\n"));
    }

    #[test]
    fn a_long_name_and_a_long_blocker_are_cut() {
        let long = "x".repeat(BLOCKER_WIDTH + 20);
        let mut card = result("a", Support::Manual, false, &[long.as_str()]);
        card.name = "n".repeat(NAME_WIDTH + 10);
        let text = render(&Report { cards: vec![card] }, &popularity());
        let line = text.lines().nth(2).unwrap_or_default();
        assert!(
            line.contains(&format!("{}...", "n".repeat(NAME_WIDTH - 3))),
            "{line}"
        );
        assert!(
            line.ends_with(&format!("{}...", "x".repeat(BLOCKER_WIDTH - 3))),
            "{line}"
        );
    }

    #[test]
    fn nothing_left_is_said_in_one_line() {
        let report = Report {
            cards: vec![
                result("a", Support::Verified, false, &[]),
                result("b", Support::Read, false, &[]),
            ],
        };
        assert_eq!(
            render(&report, &popularity()),
            "cards to draft: none, all 2 distinct cards are verified or read"
        );
    }
}
