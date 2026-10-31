//! `coverage --all`: the blockers of every Commander-legal card ranked alone, in all and by the popularity weight of
//! the cards they hold back, the share of verified cards by rank, and the list of cards for the sweep agents.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use mtg_cards::coverage::{Blocker, BlockerCount, CardResult, PreconCard, Report, analyze};
use mtg_cards::support::Support;
use mtg_db::{CardSearch, Database};

use crate::args::CoverageArgs;
use crate::audit::stored_cards_matching;
use crate::cache::Cache;
use crate::error::CliError;
use crate::popularity::Popularity;

/// How many cards, in rank order, the list written by `--out` takes its non-verified cards from.
pub const SWEEP_CARDS: usize = 2_000;

/// The rank buckets of the share table: the most played cards of the pool up to each size.
const RANK_BUCKETS: [usize; 5] = [100, 500, 1_000, 2_000, 5_000];

/// The longest a blocker runs in a table; a longer one is cut and ends in `...`.
const WIDTH: usize = 100;

/// The report on every stored card legal in Commander, as the text to print; `--out` also writes the sweep list.
pub async fn coverage_all(
    db: &Database,
    cache: &Cache,
    args: &CoverageArgs,
) -> Result<String, CliError> {
    let popularity = Popularity::from_cache(cache)?;
    let search = CardSearch {
        legal_in: Some("commander".to_owned()),
        ..CardSearch::default()
    };
    let printed = stored_cards_matching(db, &search).await?;
    if printed.is_empty() {
        return Err(CliError::Config(
            "no stored card is legal in Commander; `mtg-cli fetch scryfall` and `mtg-cli ingest` load them"
                .into(),
        ));
    }
    let cards: Vec<PreconCard> = printed
        .into_iter()
        .map(|printed| PreconCard {
            printed,
            copies: BTreeMap::new(),
        })
        .collect();
    let report = analyze(&cards, &mtg_engine::caps());
    let entries = entries(&report, &popularity);
    let mut text = render(&report, &entries, args.top);
    if let Some(path) = &args.out {
        let lines = sweep_lines(&entries, SWEEP_CARDS);
        let mut body = lines.join("\n");
        if !body.is_empty() {
            body.push('\n');
        }
        tokio::fs::write(path, body)
            .await
            .map_err(|source| CliError::io(path, source))?;
        let _ = writeln!(
            text,
            "\n{} not verified cards of the top {SWEEP_CARDS} written to {}",
            lines.len(),
            path.display()
        );
    }
    Ok(text.trim_end().to_owned())
}

/// One card of the pool with its rank and its weight.
struct Entry<'a> {
    rank: Option<u32>,
    weight: f64,
    result: &'a CardResult,
}

/// The cards of the report, the ranked ones first by rank (ties by name), then the cards without a rank by name.
fn entries<'a>(report: &'a Report, popularity: &Popularity) -> Vec<Entry<'a>> {
    let mut entries: Vec<Entry<'a>> = report
        .cards
        .iter()
        .map(|result| Entry {
            rank: popularity.rank(&result.oracle_id),
            weight: popularity.weight(&result.oracle_id),
            result,
        })
        .collect();
    // The sort is stable, so equal keys stay in name order.
    entries.sort_by_key(|entry| (entry.rank.is_none(), entry.rank));
    entries
}

fn is_verified(entry: &Entry<'_>) -> bool {
    entry.result.support == Support::Verified
}

/// How much popularity weight one blocker holds back.
#[derive(Debug, Clone, PartialEq)]
struct WeightedBlocker {
    blocker: Blocker,
    /// The summed weight of the cards it holds back, with or without other blockers.
    weight: f64,
    /// Cards it holds back.
    cards: u32,
    /// Cards it alone holds back.
    alone: u32,
    /// The lowest rank among the ranked cards it holds back.
    best: Option<u32>,
}

/// Every blocker with the weight of the cards it holds back, the heaviest first (ties: the most cards, then by name).
fn weighted_blockers(entries: &[Entry<'_>]) -> Vec<WeightedBlocker> {
    let mut found: BTreeMap<&Blocker, WeightedBlocker> = BTreeMap::new();
    for entry in entries {
        for blocker in &entry.result.blockers {
            let slot = found.entry(blocker).or_insert_with(|| WeightedBlocker {
                blocker: blocker.clone(),
                weight: 0.0,
                cards: 0,
                alone: 0,
                best: None,
            });
            slot.weight += entry.weight;
            slot.cards += 1;
            slot.alone += u32::from(entry.result.blockers.len() == 1);
            slot.best = match (slot.best, entry.rank) {
                (Some(best), Some(rank)) => Some(best.min(rank)),
                (best, rank) => best.or(rank),
            };
        }
    }
    let mut ranking: Vec<WeightedBlocker> = found.into_values().collect();
    ranking.sort_by(|a, b| {
        b.weight
            .total_cmp(&a.weight)
            .then(b.cards.cmp(&a.cards))
            .then_with(|| a.blocker.cmp(&b.blocker))
    });
    ranking
}

/// How many of the cards of one rank bucket are verified.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RankShare {
    label: String,
    cards: u32,
    verified: u32,
}

fn tally(entries: &[Entry<'_>]) -> (u32, u32) {
    let verified = entries.iter().filter(|entry| is_verified(entry)).count();
    (
        u32::try_from(entries.len()).unwrap_or(u32::MAX),
        u32::try_from(verified).unwrap_or(u32::MAX),
    )
}

/// The share of verified cards among the most played `RANK_BUCKETS` cards, among all ranked cards and among the cards
/// without a rank; `entries` is in the order of [`entries`].
fn rank_shares(entries: &[Entry<'_>]) -> Vec<RankShare> {
    let ranked = entries.partition_point(|entry| entry.rank.is_some());
    let share = |label: String, part: &[Entry<'_>]| {
        let (cards, verified) = tally(part);
        RankShare {
            label,
            cards,
            verified,
        }
    };
    let mut shares: Vec<RankShare> = RANK_BUCKETS
        .iter()
        .map(|&size| share(format!("top {size}"), &entries[..size.min(ranked)]))
        .collect();
    shares.push(share("all ranked".to_owned(), &entries[..ranked]));
    shares.push(share("no rank".to_owned(), &entries[ranked..]));
    shares
}

/// The lowercase name of a state.
const fn state_name(state: Support) -> &'static str {
    match state {
        Support::Verified => "verified",
        Support::Read => "read",
        Support::Partial => "partial",
        Support::Manual => "manual",
        Support::Stale => "stale",
        Support::Unplayable => "unplayable",
    }
}

/// A field of a line of the sweep list: tabs and line breaks become spaces.
fn field(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

/// The `rank<TAB>state<TAB>name<TAB>first blocker` lines of the cards that are not verified among the first `limit`
/// ranked cards, in rank order; the first blocker is the first of the card's blockers in report order (unread
/// clauses, then engine primitives, printed gaps and rules).
fn sweep_lines(entries: &[Entry<'_>], limit: usize) -> Vec<String> {
    entries
        .iter()
        .take(limit)
        .filter(|entry| !is_verified(entry))
        .filter_map(|entry| {
            let rank = entry.rank?;
            let blocker = entry.result.blockers.iter().next().map(Blocker::label);
            Some(format!(
                "{rank}\t{}\t{}\t{}",
                state_name(entry.result.support),
                field(&entry.result.name),
                field(&blocker.unwrap_or_default())
            ))
        })
        .collect()
}

fn cut(text: &str) -> String {
    if text.chars().count() <= WIDTH {
        return text.to_owned();
    }
    let kept: String = text.chars().take(WIDTH - 3).collect();
    format!("{kept}...")
}

fn percent(part: u32, whole: u32) -> String {
    let share = if whole == 0 {
        0.0
    } else {
        100.0 * f64::from(part) / f64::from(whole)
    };
    format!("{share:.1}%")
}

/// The report as plain text: the header, the states, the share of verified cards by rank and the three rankings.
fn render(report: &Report, entries: &[Entry<'_>], top: usize) -> String {
    let cards = u32::try_from(entries.len()).unwrap_or(u32::MAX);
    let ranked = entries.iter().filter(|entry| entry.rank.is_some()).count();
    let mut out = format!(
        "coverage: every Commander-legal card, {cards} distinct cards, {ranked} with a rank\n\n"
    );
    state_table(&mut out, report, cards);
    out.push('\n');
    share_table(&mut out, &rank_shares(entries));
    out.push('\n');
    let title = "blockers that alone hold back the most cards (alone: cards with no other blocker)";
    count_list(&mut out, title, &report.top_alone(top));
    out.push('\n');
    let title = "blockers that hold back the most cards in all (all: with or without others)";
    count_list(&mut out, title, &report.top_all(top));
    out.push('\n');
    weight_list(&mut out, entries, top);
    out
}

fn state_table(out: &mut String, report: &Report, cards: u32) {
    let _ = writeln!(out, "{:<9} {:>6} {:>6}", "state", "cards", "share");
    for row in report.tally() {
        let _ = writeln!(
            out,
            "{:<9} {:>6} {:>6}",
            row.bucket.name(),
            row.cards,
            percent(row.cards, cards)
        );
    }
}

fn share_table(out: &mut String, shares: &[RankShare]) {
    let _ = writeln!(out, "verified cards by popularity rank");
    let _ = writeln!(
        out,
        "  {:<11} {:>6} {:>9} {:>6}",
        "rank", "cards", "verified", "share"
    );
    for row in shares {
        let _ = writeln!(
            out,
            "  {:<11} {:>6} {:>9} {:>6}",
            row.label,
            row.cards,
            row.verified,
            percent(row.verified, row.cards)
        );
    }
}

fn count_list(out: &mut String, title: &str, counts: &[BlockerCount]) {
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

fn weight_list(out: &mut String, entries: &[Entry<'_>], top: usize) {
    let title = "blockers that hold back the most popularity weight (weight: share of the pool's total weight, 1 / (rank + 100); best: the lowest rank held back)";
    let _ = writeln!(out, "{title}");
    let ranking = weighted_blockers(entries);
    if ranking.is_empty() {
        out.push_str("  none\n");
        return;
    }
    let total: f64 = entries.iter().map(|entry| entry.weight).sum();
    let _ = writeln!(
        out,
        "  {:>7} {:>6} {:>6} {:>6}  blocker",
        "weight", "cards", "alone", "best"
    );
    for row in ranking.iter().take(top) {
        let best = row
            .best
            .map_or_else(|| "-".to_owned(), |rank| rank.to_string());
        let _ = writeln!(
            out,
            "  {:>6.2}% {:>6} {:>6} {:>6}  {}",
            100.0 * row.weight / total,
            row.cards,
            row.alone,
            best,
            cut(&row.blocker.label())
        );
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the pool ranks its cards by popularity with the unranked last, weighs each blocker by the cards it holds back, counts the verified share of the rank buckets, and lists the cards that are not verified among the top ones for the sweep agents.
    use std::collections::BTreeSet;

    use rstest::rstest;

    use mtg_cards::coverage::RulesSource;

    use super::*;

    fn result(id: &str, support: Support, blockers: &[&str]) -> CardResult {
        CardResult {
            oracle_id: id.to_owned(),
            name: format!("Card {id}"),
            rules: RulesSource::Compiled,
            support,
            blockers: blockers
                .iter()
                .map(|shape| Blocker::Clause((*shape).to_owned()))
                .collect::<BTreeSet<_>>(),
            copies: BTreeMap::new(),
        }
    }

    /// Card `a` has rank 1, `b` 2, `c` 3, `d` 4 and `e` none; `ranks` are built from these.
    fn popularity() -> Popularity {
        let ranks = [("a", 1), ("b", 2), ("c", 3), ("d", 4)];
        Popularity::new(
            ranks
                .into_iter()
                .map(|(id, n)| (id.to_owned(), n))
                .collect(),
        )
        .unwrap()
    }

    fn report() -> Report {
        Report {
            cards: vec![
                result("a", Support::Verified, &[]),
                result("b", Support::Partial, &["x"]),
                result("c", Support::Manual, &["x", "y"]),
                result("d", Support::Partial, &["y"]),
                result("e", Support::Partial, &["z"]),
            ],
        }
    }

    fn ids(entries: &[Entry<'_>]) -> String {
        entries
            .iter()
            .map(|entry| entry.result.oracle_id.as_str())
            .collect()
    }

    #[test]
    fn cards_are_put_in_rank_order_and_those_without_a_rank_last_by_name() {
        let report = Report {
            cards: ["a", "b", "c", "d", "e", "f"]
                .iter()
                .map(|id| result(id, Support::Manual, &["x"]))
                .collect(),
        };
        let ranks = [("d", 1), ("b", 2), ("a", 3), ("c", 4)];
        let ranks = ranks
            .into_iter()
            .map(|(id, n)| (id.to_owned(), n))
            .collect();

        let entries = entries(&report, &Popularity::new(ranks).unwrap());

        assert_eq!(ids(&entries), "dbacef");
        assert_eq!(
            entries.iter().map(|e| e.rank).collect::<Vec<_>>(),
            [Some(1), Some(2), Some(3), Some(4), None, None]
        );
    }

    #[test]
    fn a_card_without_a_rank_weighs_as_the_last_rank() {
        let report = report();
        let entries = entries(&report, &popularity());
        let weight = |id: &str| {
            entries
                .iter()
                .find(|e| e.result.oracle_id == id)
                .map(|e| e.weight)
                .unwrap()
        };
        assert!((weight("a") - 1.0 / 101.0).abs() < 1e-12);
        assert!((weight("d") - 1.0 / 104.0).abs() < 1e-12);
        assert!((weight("e") - weight("d")).abs() < 1e-12);
    }

    #[test]
    fn blockers_are_ranked_by_the_summed_weight_of_the_cards_they_hold_back() {
        let report = report();
        let entries = entries(&report, &popularity());

        let ranking = weighted_blockers(&entries);

        let row = |shape: &str| {
            ranking
                .iter()
                .find(|row| row.blocker == Blocker::Clause(shape.to_owned()))
                .unwrap()
        };
        // x holds back b (rank 2) and c (rank 3); y holds back c (3) and d (4); z holds back e, which has no rank.
        assert!((row("x").weight - (1.0 / 102.0 + 1.0 / 103.0)).abs() < 1e-12);
        assert!((row("y").weight - (1.0 / 103.0 + 1.0 / 104.0)).abs() < 1e-12);
        assert!((row("z").weight - 1.0 / 104.0).abs() < 1e-12);
        assert_eq!(
            ranking
                .iter()
                .map(|r| r.blocker.label())
                .collect::<Vec<_>>(),
            ["x", "y", "z"]
        );
        assert_eq!(
            (row("x").cards, row("x").alone, row("x").best),
            (2, 1, Some(2))
        );
        assert_eq!(
            (row("y").cards, row("y").alone, row("y").best),
            (2, 1, Some(3))
        );
        assert_eq!(
            (row("z").cards, row("z").alone, row("z").best),
            (1, 1, None)
        );
    }

    #[test]
    fn many_light_cards_outweigh_one_heavy_card_only_when_their_weights_add_up_to_more() {
        // 30 unranked cards behind `tail` (weight 1/104 each) beat the one card of rank 1 behind `head` (1/101).
        let mut cards = vec![result("a", Support::Partial, &["head"])];
        cards.extend((0..30).map(|n| result(&format!("t{n:02}"), Support::Partial, &["tail"])));
        let report = Report { cards };
        let entries = entries(&report, &popularity());

        let ranking = weighted_blockers(&entries);

        assert_eq!(ranking[0].blocker.label(), "tail");
        assert_eq!(ranking[1].blocker.label(), "head");
        assert_eq!(ranking[1].best, Some(1));
    }

    #[rstest]
    #[case::none_verified(&[Support::Partial, Support::Manual], 0)]
    #[case::all_verified(&[Support::Verified, Support::Verified], 2)]
    #[case::one_of_two(&[Support::Verified, Support::Stale], 1)]
    fn a_bucket_counts_its_verified_cards(#[case] states: &[Support], #[case] verified: u32) {
        let cards: Vec<CardResult> = states
            .iter()
            .enumerate()
            .map(|(n, &state)| result(&format!("id{n}"), state, &["x"]))
            .collect();
        let report = Report { cards };
        let ranks: BTreeMap<String, u32> = (0..states.len())
            .map(|n| (format!("id{n}"), u32::try_from(n + 1).unwrap()))
            .collect();
        let entries = entries(&report, &Popularity::new(ranks).unwrap());

        let shares = rank_shares(&entries);

        let all = shares.iter().find(|s| s.label == "all ranked").unwrap();
        assert_eq!(
            (all.cards, all.verified),
            (u32::try_from(states.len()).unwrap(), verified)
        );
    }

    #[test]
    fn the_rank_buckets_take_the_most_played_cards_of_the_pool() {
        // 120 ranked cards: the first 100 verified, the rest not; and two cards without a rank, one verified.
        let mut cards: Vec<CardResult> = (0..120)
            .map(|n| {
                let support = if n < 100 {
                    Support::Verified
                } else {
                    Support::Manual
                };
                let blockers: &[&str] = if n < 100 { &[] } else { &["x"] };
                result(&format!("r{n:03}"), support, blockers)
            })
            .collect();
        cards.push(result("u1", Support::Verified, &[]));
        cards.push(result("u2", Support::Manual, &["x"]));
        let ranks = (0..120).map(|n| (format!("r{n:03}"), n + 1)).collect();
        let report = Report { cards };
        let entries = entries(&report, &Popularity::new(ranks).unwrap());

        let shares: Vec<(String, u32, u32)> = rank_shares(&entries)
            .into_iter()
            .map(|s| (s.label, s.cards, s.verified))
            .collect();

        let row = |label: &str, cards, verified| (label.to_owned(), cards, verified);
        assert_eq!(
            shares,
            [
                row("top 100", 100, 100),
                row("top 500", 120, 100),
                row("top 1000", 120, 100),
                row("top 2000", 120, 100),
                row("top 5000", 120, 100),
                row("all ranked", 120, 100),
                row("no rank", 2, 1),
            ]
        );
    }

    #[test]
    fn the_sweep_list_has_the_not_verified_cards_of_the_top_in_rank_order() {
        let report = report();
        let entries = entries(&report, &popularity());

        let lines = sweep_lines(&entries, 3);

        // The top three are a (verified), b and c; the first blocker is the first in the card's order.
        assert_eq!(lines, ["2\tpartial\tCard b\tx", "3\tmanual\tCard c\tx"]);
    }

    #[test]
    fn the_sweep_list_leaves_out_cards_without_a_rank_and_cleans_its_fields() {
        let mut odd = result("a", Support::Partial, &["two\twords\nand more"]);
        odd.name = "Tab\there".to_owned();
        let report = Report {
            cards: vec![odd, result("e", Support::Manual, &["x"])],
        };
        let entries = entries(&report, &popularity());

        let lines = sweep_lines(&entries, 2_000);

        assert_eq!(lines, ["1\tpartial\tTab here\ttwo words and more"]);
    }

    #[rstest]
    #[case(Support::Verified, "verified")]
    #[case(Support::Partial, "partial")]
    #[case(Support::Manual, "manual")]
    #[case(Support::Stale, "stale")]
    #[case(Support::Unplayable, "unplayable")]
    fn a_state_is_named_as_the_report_names_it(#[case] state: Support, #[case] name: &str) {
        assert_eq!(state_name(state), name);
    }

    #[test]
    fn the_report_shows_the_states_the_buckets_and_the_three_rankings() {
        let report = report();
        let entries = entries(&report, &popularity());

        let text = render(&report, &entries, 2);

        assert!(
            text.starts_with(
                "coverage: every Commander-legal card, 5 distinct cards, 4 with a rank\n"
            ),
            "{text}"
        );
        for wanted in [
            "state      cards  share",
            "verified cards by popularity rank",
            "all ranked",
            "blockers that alone hold back the most cards",
            "blockers that hold back the most cards in all",
            "blockers that hold back the most popularity weight",
        ] {
            assert!(text.contains(wanted), "{wanted} missing from\n{text}");
        }
        // `--top 2` cuts each of the three rankings to two rows, so the third blocker, z, is in none.
        let rows = |blocker: &str| text.lines().filter(|l| l.ends_with(blocker)).count();
        assert_eq!((rows("  x"), rows("  y"), rows("  z")), (3, 3, 0), "{text}");
    }
}
