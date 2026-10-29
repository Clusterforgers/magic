//! `audit`: a seeded sample of the stored cards the compiler reads, each listed as its oracle text beside the rendered reading and the JSON, so a person can check the templates against the cards.

use std::fmt::Write as _;

use mtg_cards::ability::Ability;
use mtg_cards::card::ImplStatus;
use mtg_cards::compile::{Clause, ClauseKind, Compiled, compile, render_variants, same_text};
use mtg_cards::{PrintedCard, PrintedFace};
use mtg_db::cards::MAX_PAGE_SIZE;
use mtg_db::{CardSearch, Database, Page};

use crate::args::AuditArgs;
use crate::coverage::printed_card;
use crate::error::CliError;
use crate::status::missing_table;

/// One card of the sample with what the compiler made of it.
#[derive(Debug)]
pub struct Entry<'a> {
    /// The card as stored.
    pub printed: &'a PrintedCard,
    /// Its compiled rules and clauses.
    pub compiled: Compiled,
}

/// The sample as text, one block per card, preceded by a line naming the seed and the count.
pub async fn audit(db: &Database, args: &AuditArgs) -> Result<String, CliError> {
    let cards = stored_cards(db).await?;
    if cards.is_empty() {
        return Err(CliError::Config(
            "no cards are stored; `mtg-cli fetch scryfall` and `mtg-cli ingest` load them".into(),
        ));
    }
    let entries = sample(&cards, args.count, args.seed);
    Ok(render(&entries, args.seed))
}

/// Every stored deck card, in name order.
pub(crate) async fn stored_cards(db: &Database) -> Result<Vec<PrintedCard>, CliError> {
    stored_cards_matching(db, &CardSearch::default()).await
}

/// The stored cards a search selects, in its order.
pub(crate) async fn stored_cards_matching(
    db: &Database,
    search: &CardSearch,
) -> Result<Vec<PrintedCard>, CliError> {
    let repo = db.cards();
    let mut cards = Vec::new();
    let mut page = Page::new(MAX_PAGE_SIZE, 0);
    loop {
        let found = repo.search(search, page).await.map_err(|error| {
            if missing_table(&error) {
                CliError::Config(
                    "the schema is not applied yet; `mtg-cli ingest` applies it".into(),
                )
            } else {
                CliError::Db(error)
            }
        })?;
        if found.cards.is_empty() {
            return Ok(cards);
        }
        cards.extend(found.cards.iter().map(printed_card));
        page = page.next();
    }
}

/// Whether the compiler reads something of the card beyond keyword lines: a card with only its
/// keywords read shows nothing of the templates.
fn is_audited(compiled: &Compiled) -> bool {
    compiled
        .card
        .as_ref()
        .is_some_and(|card| card.impl_status != ImplStatus::Manual)
        && compiled
            .clauses
            .iter()
            .any(|clause| clause.read && clause.kind != ClauseKind::Keyword)
}

/// One step of the `SplitMix64` mixer: a well mixed word of the state.
fn mix(state: u64) -> u64 {
    let state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut word = state;
    word = (word ^ (word >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    word = (word ^ (word >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    word ^ (word >> 31)
}

/// The place of a card in the seed's order: its oracle id (FNV-1a) mixed with the seed.
pub(crate) fn rank(seed: u64, oracle_id: &str) -> u64 {
    let hash = oracle_id
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    mix(mix(seed) ^ hash)
}

/// At most `count` cards the compiler reads, chosen by the seed alone: the cards are put in the
/// seed's order and the first `count` that are read are taken, so the same seed gives the same
/// sample whatever order the cards come in, a smaller count is a prefix of a larger one, and a
/// card that stops being read is replaced by the next. The result is in name order.
pub fn sample(cards: &[PrintedCard], count: usize, seed: u64) -> Vec<Entry<'_>> {
    let mut order: Vec<&PrintedCard> = cards.iter().collect();
    order.sort_by(|a, b| {
        (rank(seed, &a.oracle_id), &a.oracle_id).cmp(&(rank(seed, &b.oracle_id), &b.oracle_id))
    });
    let mut chosen: Vec<Entry<'_>> = order
        .into_iter()
        .filter_map(|printed| {
            let compiled = compile(printed);
            is_audited(&compiled).then_some(Entry { printed, compiled })
        })
        .take(count)
        .collect();
    chosen.sort_by(|a, b| {
        (&a.printed.name, &a.printed.oracle_id).cmp(&(&b.printed.name, &b.printed.oracle_id))
    });
    chosen
}

/// The listing: a heading line, then each card's block.
pub fn render(entries: &[Entry<'_>], seed: u64) -> String {
    let mut text = format!("audit: seed {seed}, {} cards\n", entries.len());
    for entry in entries {
        text.push('\n');
        text.push_str(&card_block(entry));
    }
    text.trim_end().to_owned()
}

/// A card: its name, cost, type line and status, then each face's clauses.
fn card_block(entry: &Entry<'_>) -> String {
    let status = entry
        .compiled
        .card
        .as_ref()
        .map_or(ImplStatus::Manual, |card| card.impl_status);
    let read = entry.compiled.clauses.iter().filter(|c| c.read).count();
    let mut text = format!(
        "## {} [{status:?} {read}/{}] {}\n",
        entry.printed.name,
        entry.compiled.clauses.len(),
        entry.printed.oracle_id
    );
    for (index, face) in entry.printed.faces.iter().enumerate() {
        text.push_str(&face_block(entry, index, face));
    }
    text
}

/// The clauses of one face in text order. A read clause shows the oracle text, the reading
/// (`render_variants`, the wording that equals the clause) and the JSON; an unread clause shows its text.
fn face_block(entry: &Entry<'_>, index: usize, face: &PrintedFace) -> String {
    let mut text = format!(
        "# {} {} | {}\n",
        face.name,
        face.mana_cost,
        face.type_line.replace('\u{2014}', "-")
    );
    let clauses: Vec<&Clause> = entry
        .compiled
        .clauses
        .iter()
        .filter(|clause| clause.face == index)
        .collect();
    let abilities = entry
        .compiled
        .card
        .as_ref()
        .and_then(|card| card.faces.get(index))
        .map(|face| face.abilities.as_slice())
        .unwrap_or_default();
    let mut next = abilities.iter();
    let mut spell_shown = false;
    for clause in &clauses {
        let source = one_line(clause.span.slice(&face.oracle_text).unwrap_or(&clause.text));
        if !clause.read {
            next.next();
            let _ = writeln!(text, "  unread  {source}");
            continue;
        }
        // The spell lines of a card are one ability, shown with the first of them.
        if clause.kind == ClauseKind::Spell {
            if spell_shown {
                continue;
            }
            spell_shown = true;
        }
        let Some(ability) = next.next() else {
            let _ = writeln!(text, "  oracle  {source}\n  reads   (no ability)");
            continue;
        };
        let oracle = if clause.kind == ClauseKind::Spell {
            spell_source(&clauses, face)
        } else {
            source
        };
        text.push_str(&read_lines(&oracle, clause, ability));
    }
    text
}

/// The oracle text of all the spell lines of a face, in one line.
fn spell_source(clauses: &[&Clause], face: &PrintedFace) -> String {
    clauses
        .iter()
        .filter(|clause| clause.kind == ClauseKind::Spell)
        .map(|clause| one_line(clause.span.slice(&face.oracle_text).unwrap_or(&clause.text)))
        .collect::<Vec<_>>()
        .join(" / ")
}

/// The lines of a read clause.
fn read_lines(oracle: &str, clause: &Clause, ability: &Ability) -> String {
    let json = serde_json::to_string(ability).unwrap_or_else(|e| format!("(unwritable: {e})"));
    if clause.kind == ClauseKind::Keyword {
        return format!("  keyword {oracle}\n  json    {json}\n");
    }
    let reading = reading(clause, ability);
    format!("  oracle  {oracle}\n  reads   {reading}\n  json    {json}\n")
}

/// The wording of the ability that equals the clause (the one that let it count as read).
fn reading(clause: &Clause, ability: &Ability) -> String {
    let variants = render_variants(ability);
    variants
        .iter()
        .find(|written| same_text(written, &clause.text))
        .or(variants.first())
        .map_or_else(|| "(no wording)".to_owned(), |written| one_line(written))
}

/// The text with its line breaks as ` / `.
fn one_line(text: &str) -> String {
    text.lines().collect::<Vec<_>>().join(" / ")
}

#[cfg(test)]
mod tests {
    //! Goal: the audit sample is decided by the seed alone (same seed, same cards, whatever order the cards come in), holds only cards the compiler reads beyond keyword lines, never more than the count, and each block shows the oracle text, the reading and the JSON of a read clause and the text of an unread one.
    use std::collections::BTreeSet;

    use rstest::rstest;

    use super::*;

    fn card(n: usize, text: &str) -> PrintedCard {
        PrintedCard {
            layout: String::new(),
            oracle_id: format!("00000000-0000-4000-8000-{n:012}"),
            name: format!("Card {n:03}"),
            keywords: vec![],
            faces: vec![PrintedFace {
                name: format!("Card {n:03}"),
                mana_cost: "{1}{U}".into(),
                type_line: "Creature \u{2014} Wizard".into(),
                oracle_text: text.into(),
                power: Some("1".into()),
                toughness: Some("1".into()),
                ..PrintedFace::default()
            }],
        }
    }

    /// Sixty cards of four kinds: a trigger the templates read, only keywords, an unread ability, no text.
    fn pool() -> Vec<PrintedCard> {
        let texts = [
            "When this creature enters, draw a card.",
            "Flying\nVigilance",
            "Whenever you cast a spell, wish for a card.",
            "",
        ];
        (0..60).map(|n| card(n, texts[n % 4])).collect()
    }

    fn names(entries: &[Entry<'_>]) -> Vec<String> {
        entries.iter().map(|e| e.printed.name.clone()).collect()
    }

    #[test]
    fn the_same_seed_gives_the_same_sample_in_any_input_order() {
        let cards = pool();
        let mut reversed = cards.clone();
        reversed.reverse();
        let first = names(&sample(&cards, 8, 7));
        assert_eq!(first, names(&sample(&cards, 8, 7)));
        assert_eq!(first, names(&sample(&reversed, 8, 7)));
    }

    #[rstest]
    #[case::one(1)]
    #[case::ten(10)]
    #[case::all_there_are(15)]
    fn the_sample_is_the_count_of_compiled_cards(#[case] count: usize) {
        let cards = pool();
        let entries = sample(&cards, count, 3);
        assert_eq!(entries.len(), count);
        assert!(entries.iter().all(|e| is_audited(&e.compiled)));
        let distinct: BTreeSet<_> = names(&entries).into_iter().collect();
        assert_eq!(distinct.len(), count, "a card appears once");
    }

    #[test]
    fn a_count_beyond_the_compiled_cards_gives_all_of_them_and_only_them() {
        let cards = pool();
        let entries = sample(&cards, 100, 3);
        // Only the draw-a-card trigger is read beyond keywords: 15 of the 60.
        assert_eq!(entries.len(), 15);
        assert!(entries.iter().all(|e| {
            e.printed
                .faces
                .iter()
                .all(|f| f.oracle_text.starts_with("When this creature enters"))
        }));
    }

    #[test]
    fn other_seeds_give_other_samples_and_a_smaller_count_is_a_prefix_of_the_order() {
        let cards = pool();
        let a = names(&sample(&cards, 5, 1));
        assert_ne!(a, names(&sample(&cards, 5, 2)));
        // Every card of the smaller sample is in the larger one.
        let larger: BTreeSet<_> = names(&sample(&cards, 9, 1)).into_iter().collect();
        assert!(a.iter().all(|name| larger.contains(name)));
    }

    #[test]
    fn the_sample_is_listed_in_name_order() {
        let cards = pool();
        let listed = names(&sample(&cards, 10, 5));
        let mut sorted = listed.clone();
        sorted.sort();
        assert_eq!(listed, sorted);
    }

    #[test]
    fn a_card_whose_only_reading_is_a_keyword_line_is_not_sampled() {
        let cards = vec![card(1, "Flying\nVigilance"), card(2, "Haste")];
        assert!(sample(&cards, 5, 1).is_empty());
    }

    fn block(text: &str) -> String {
        let cards = vec![card(1, text)];
        render(&sample(&cards, 1, 1), 1)
    }

    #[test]
    fn a_read_clause_shows_the_oracle_text_the_reading_and_the_json() {
        let text = block("When this creature enters, draw a card. (Reminder.)");
        assert!(
            text.starts_with("audit: seed 1, 1 cards\n\n## Card 001 [Full 1/1] "),
            "{text}"
        );
        assert!(
            text.contains("# Card 001 {1}{U} | Creature - Wizard\n"),
            "{text}"
        );
        assert!(
            text.contains("  oracle  When this creature enters, draw a card. (Reminder.)\n"),
            "{text}"
        );
        assert!(
            text.contains("  reads   When ~ enters, draw a card.\n"),
            "{text}"
        );
        assert!(text.contains("  json    {\"Triggered\":"), "{text}");
    }

    #[test]
    fn an_unread_clause_is_listed_with_its_text_and_the_status_is_partial() {
        let text = block(
            "When this creature enters, draw a card.\nWhenever you cast a spell, wish for a card.",
        );
        assert!(text.contains("[Partial 1/2] "), "{text}");
        assert!(
            text.ends_with("  unread  Whenever you cast a spell, wish for a card."),
            "{text}"
        );
    }

    #[test]
    fn the_lines_of_a_spell_are_one_reading() {
        let cards = vec![PrintedCard {
            layout: String::new(),
            oracle_id: "00000000-0000-4000-8000-000000000009".into(),
            name: "Study".into(),
            keywords: vec![],
            faces: vec![PrintedFace {
                name: "Study".into(),
                mana_cost: "{1}{U}".into(),
                type_line: "Sorcery".into(),
                oracle_text: "Draw a card.\nYou gain 2 life.".into(),
                ..PrintedFace::default()
            }],
        }];
        let text = render(&sample(&cards, 1, 1), 1);
        assert_eq!(text.matches("  json    ").count(), 1, "{text}");
        assert!(
            text.contains("  oracle  Draw a card. / You gain 2 life.\n"),
            "{text}"
        );
    }
}
