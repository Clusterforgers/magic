//! The plain-text report: the states of the cards, the best and the worst decks, and the ranked blockers.

use std::fmt::Write as _;

use super::{BlockerCount, DeckShare, Report, StateTally};

/// The longest a shape or a deck label runs; a longer one is cut and ends in `...`.
const WIDTH: usize = 100;

/// How many decks the best list and the worst list show at most.
const DECKS_SHOWN: usize = 5;

/// How many blockers each ranking shows unless asked otherwise.
pub const DEFAULT_TOP: usize = 20;

fn cut(text: &str) -> String {
    if text.chars().count() <= WIDTH {
        return text.to_owned();
    }
    let kept: String = text.chars().take(WIDTH - 3).collect();
    format!("{kept}...")
}

fn counted(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn percent(part: u32, whole: u32) -> String {
    let share = if whole == 0 {
        0.0
    } else {
        100.0 * f64::from(part) / f64::from(whole)
    };
    format!("{share:.1}%")
}

impl Report {
    /// The report as plain text of about 80 lines: the cards by state, the best and the worst decks by
    /// the share of their copies that are `Verified`, and the `top` blockers that alone hold back the
    /// most cards, then the `top` that hold back the most in all.
    pub fn render(&self, top: usize) -> String {
        let decks = self.decks();
        let tally = self.tally();
        let cards: u32 = tally.iter().map(|row| row.cards).sum();
        let copies: u32 = tally.iter().map(|row| row.copies).sum();
        let mut out = format!(
            "coverage: {}, {}, {}\n\n",
            counted(
                u64::try_from(decks.len()).unwrap_or(u64::MAX),
                "precon",
                "precons"
            ),
            counted(u64::from(cards), "distinct card", "distinct cards"),
            counted(u64::from(copies), "copy", "copies")
        );
        state_table(&mut out, &tally, cards, copies);
        let best = decks.len().div_ceil(2).min(DECKS_SHOWN);
        let worst = (decks.len() - best).min(DECKS_SHOWN);
        if best > 0 {
            out.push('\n');
            let title = "best precons by the share of their cards that are verified";
            deck_list(&mut out, title, decks[..best].iter());
        }
        if worst > 0 {
            out.push('\n');
            let title = "worst precons by the share of their cards that are verified";
            deck_list(&mut out, title, decks[decks.len() - worst..].iter().rev());
        }
        out.push('\n');
        let title =
            "blockers that alone hold back the most cards (alone: cards with no other blocker)";
        blocker_list(&mut out, title, &self.top_alone(top));
        out.push('\n');
        let title = "blockers that hold back the most cards in all (all: with or without others)";
        blocker_list(&mut out, title, &self.top_all(top));
        out
    }
}

fn state_table(out: &mut String, tally: &[StateTally], cards: u32, copies: u32) {
    let _ = writeln!(
        out,
        "{:<9} {:>6} {:>6} {:>8} {:>6}",
        "state", "cards", "share", "copies", "share"
    );
    for row in tally {
        let _ = writeln!(
            out,
            "{:<9} {:>6} {:>6} {:>8} {:>6}",
            row.bucket.name(),
            row.cards,
            percent(row.cards, cards),
            row.copies,
            percent(row.copies, copies)
        );
    }
}

fn deck_list<'a>(out: &mut String, title: &str, decks: impl Iterator<Item = &'a DeckShare>) {
    let _ = writeln!(out, "{title}");
    for deck in decks {
        let _ = writeln!(
            out,
            "  {:>6} {:>9}  {}",
            percent(deck.verified, deck.copies),
            format!("{}/{}", deck.verified, deck.copies),
            cut(&deck.deck)
        );
    }
}

fn blocker_list(out: &mut String, title: &str, counts: &[BlockerCount]) {
    let _ = writeln!(out, "{title}");
    if counts.is_empty() {
        out.push_str("  none\n");
        return;
    }
    let _ = writeln!(out, "  {:>6} {:>6}  blocker", "alone", "all");
    for count in counts {
        let _ = writeln!(
            out,
            "  {:>6} {:>6}  {}",
            count.alone,
            count.in_all,
            cut(&count.blocker.label())
        );
    }
}
