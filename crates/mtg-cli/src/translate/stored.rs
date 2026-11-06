//! The stored cards a run is restricted to (`--stored`, `--precons`), and what the compiler and the corpus already make of
//! each.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use mtg_cards::builtin;
use mtg_cards::coverage::analyze_with;
use mtg_cards::generated::{Generation, generate};
use mtg_cards::load::{Library, slug, unwhole};
use mtg_cards::proposal::{ClauseState, Report, check_exact};
use mtg_cards::support::{Caps, PrintedCard, Support, support};
use mtg_cards::validate::used_primitives;
use mtg_db::Database;

use crate::audit::stored_cards;
use crate::compile::names_of;
use crate::coverage::precon_cards;
use crate::error::CliError;

/// The stored cards by slug, with what is needed to tell what the rules of the corpus and the compiler already give.
pub struct Stored {
    cards: BTreeMap<String, PrintedCard>,
    library: &'static Library,
    names: BTreeSet<String>,
    caps: Caps,
    /// With `--precons`: the slugs of the precon cards with the report's bucket and first blocker of each.
    precons: Option<BTreeMap<String, (String, String)>>,
}

impl Stored {
    /// Reads the stored cards; with `precons`, only the cards of the stored precons count, judged as `coverage` judges them.
    pub async fn load(db: &Database, precons: bool) -> Result<Stored, CliError> {
        let library = builtin::library().map_err(|errors| {
            CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
        })?;
        let printed = stored_cards(db).await?;
        let names = names_of(&printed);
        let cards = printed
            .into_iter()
            .map(|card| (slug(&card.name), card))
            .collect();
        let caps = mtg_engine::caps();
        let precons = if precons {
            let (cards, _) = precon_cards(db, None).await?;
            let report = analyze_with(&cards, library, &caps);
            Some(
                report
                    .cards
                    .iter()
                    .map(|card| {
                        let blocker = card
                            .blockers
                            .iter()
                            .next()
                            .map(mtg_cards::coverage::Blocker::label);
                        (
                            slug(&card.name),
                            (card.bucket().name().to_owned(), blocker.unwrap_or_default()),
                        )
                    })
                    .collect(),
            )
        } else {
            None
        };
        Ok(Stored {
            cards,
            library,
            names,
            caps,
            precons,
        })
    }

    /// The slugs of every card the database stores.
    pub fn slugs(&self) -> BTreeSet<String> {
        self.cards.keys().cloned().collect()
    }

    /// Whether the run keeps a card with this slug: the database stores it (and with `--precons`, a precon holds it).
    pub fn has(&self, slug: &str) -> bool {
        self.cards.contains_key(slug)
            && self
                .precons
                .as_ref()
                .is_none_or(|precons| precons.contains_key(slug))
    }

    /// The printed Oracle text of each face of the stored card with this slug.
    pub fn printed_oracle(&self, slug: &str) -> Option<Vec<&str>> {
        let card = self.cards.get(slug)?;
        Some(
            card.faces
                .iter()
                .map(|face| face.oracle_text.as_str())
                .collect(),
        )
    }

    /// The oracle id of the stored card with this slug.
    pub fn oracle_id(&self, slug: &str) -> Option<&str> {
        self.cards.get(slug).map(|card| card.oracle_id.as_str())
    }

    /// The precon cards, one `slug<TAB>bucket<TAB>first blocker` line each, in slug order; none without `--precons`.
    pub fn precon_lines(&self) -> Option<String> {
        let precons = self.precons.as_ref()?;
        let mut text = String::from("slug\tstate\tfirst blocker\n");
        for (slug, (bucket, blocker)) in precons {
            let _ = writeln!(text, "{slug}\t{bucket}\t{blocker}");
        }
        Some(text)
    }

    /// Whether `promote --draft` would take the proposal (without the smoke gate): the exact check gives a whole
    /// rules file the engine runs as `Verified`.
    pub fn offered(&self, slug: &str, proposal: &str) -> Option<bool> {
        self.offer(slug, proposal)
            .map(|offer| offer.refused.is_none())
    }

    /// Whether the proposal is offered and why not, with the constructs of its clauses that have no read-back writer:
    /// the offer trusts the translation for them, because the draft path takes an unchecked verdict of a Forge script
    /// that matches the text.
    pub fn offer(&self, slug: &str, proposal: &str) -> Option<Offer> {
        let printed = self.cards.get(slug)?;
        let report = check_exact(proposal, printed, self.library, &self.names, &self.caps);
        Some(Offer {
            refused: refusal(&report, &self.caps),
            unwritten: report.unwritten().map(str::to_owned).collect(),
            differs: report
                .clauses
                .iter()
                .filter_map(|clause| match &clause.state {
                    ClauseState::Differs { written, .. } => {
                        Some((clause.text.clone(), written.clone()))
                    }
                    _ => None,
                })
                .collect(),
        })
    }

    /// What the corpus or the compiler makes of the card today: `corpus-verified`, `compiled-verified`, `partial`,
    /// `manual` and the like, the words of the support states; with `--precons`, the bucket `coverage` puts the card in.
    pub fn today(&self, slug: &str) -> String {
        if let Some(precons) = &self.precons {
            return precons
                .get(slug)
                .map_or_else(|| "unknown".to_owned(), |(bucket, _)| bucket.clone());
        }
        let Some(printed) = self.cards.get(slug) else {
            return "unknown".to_owned();
        };
        if let Some(card) = self.library.card(&printed.oracle_id) {
            let state = support(Some(card), printed, &self.caps).support;
            return format!("corpus-{state:?}").to_lowercase();
        }
        match generate(printed, self.library, &self.names) {
            Generation::Rules(card) => {
                let state = support(Some(&card), printed, &self.caps).support;
                format!("compiled-{state:?}").to_lowercase()
            }
            Generation::Nothing => "none".to_owned(),
            Generation::Invalid(_) => "invalid".to_owned(),
        }
    }
}

/// What the offer makes of a whole translation.
pub struct Offer {
    /// Why the proposal is not offered, as a gap name (`offer:...`); `None` when it is.
    pub refused: Option<String>,
    /// The constructs of its clauses that have no read-back writer.
    pub unwritten: Vec<String>,
    /// The clauses written back as other text, each with the wording of its ability.
    pub differs: Vec<(String, String)>,
}

/// Why the exact check refuses a whole translation, as a gap name: the first problem (`offer:invalid:`), a printed
/// clause no ability stands for (`offer:open:` and its kind), the primitives the engine does not run (`offer:engine:`),
/// or another state than `Verified`; `None` when the rules are whole and `Verified`.
pub fn refusal(report: &Report, caps: &Caps) -> Option<String> {
    if let Some(problem) = report.problems.first() {
        return Some(format!("offer:invalid:{}", shape(&problem.message)));
    }
    let (Some(record), Some(rules)) = (&report.support, &report.rules) else {
        return Some("offer:invalid:no-rules".to_owned());
    };
    if let Some(open) = report
        .clauses
        .iter()
        .find(|clause| clause.state == ClauseState::Open)
    {
        return Some(format!("offer:open:{:?}", open.kind));
    }
    let missing: BTreeSet<String> = rules
        .abilities()
        .flat_map(|(_, ability)| caps.missing(&used_primitives(ability)))
        .collect();
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(String::as_str).collect();
        return Some(format!("offer:engine:{}", names.join(",")));
    }
    if record.support != Support::Verified {
        return Some(format!(
            "offer:{}",
            format!("{:?}", record.support).to_lowercase()
        ));
    }
    unwhole(rules).map(|_| "offer:unwhole".to_owned())
}

/// A message as a gap name: one line without the separators of the cards file, with quoted text and numbers folded so
/// that cards refused for the same reason share it.
pub fn shape(message: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    for c in message.chars() {
        match c {
            '"' | '`' => {
                quoted = !quoted;
                if quoted {
                    out.push_str("`_`");
                }
            }
            _ if quoted => {}
            '0'..='9' if out.ends_with('#') => {}
            '0'..='9' => out.push('#'),
            '\t' | '\n' | ';' => out.push(' '),
            _ => out.push(c),
        }
    }
    out.trim().to_owned()
}
