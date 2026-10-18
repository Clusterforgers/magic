//! Support records: whether the game can play a card without manual steps, computed from its
//! rules file, its imported oracle text and the primitives the engine runs. No engine is needed.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assemble::printed_gaps;
use crate::card::{Card, ImplStatus, Span};
use crate::validate::used_primitives;

/// A face of a card as imported (the strings Scryfall gives, kept as text).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrintedFace {
    /// Face name.
    pub name: String,
    /// Mana cost text such as `{X}{R}`; empty when the face has none.
    pub mana_cost: String,
    /// Type line such as `Legendary Creature — Beast`.
    pub type_line: String,
    /// Oracle text, lines joined by newlines.
    pub oracle_text: String,
    /// Printed power, as text (it can be `*`).
    pub power: Option<String>,
    /// Printed toughness, as text.
    pub toughness: Option<String>,
    /// Starting loyalty, as text.
    pub loyalty: Option<String>,
    /// Battle defense, as text.
    pub defense: Option<String>,
    /// The face's colors as Scryfall letters (`W`, `U`, `B`, `R`, `G`); empty when the record gives none.
    pub colors: Vec<String>,
}

/// An oracle card as imported: what the rules file is checked against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrintedCard {
    /// The Scryfall oracle id.
    pub oracle_id: String,
    /// The full card name.
    pub name: String,
    /// Scryfall's keyword list (it includes keyword actions and ability words).
    pub keywords: Vec<String>,
    /// The faces in Scryfall's order.
    pub faces: Vec<PrintedFace>,
}

/// Whether and how well the game plays a card (`docs/deck-check.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Support {
    /// Rules exist, cover the text, the text is unchanged and the engine runs every primitive used.
    Verified,
    /// Rules exist except the listed clauses.
    Partial,
    /// No rules, or the engine runs none of them: the table resolves the card by hand.
    Manual,
    /// Rules exist but the oracle text changed since they were written; counts as `Manual`.
    Stale,
    /// The engine cannot host the card.
    Unplayable,
}

/// A clause of the oracle text the game cannot run, with the reason in plain words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clause {
    /// Which face.
    pub face: usize,
    /// Where in that face's oracle text.
    pub span: Span,
    /// Why it is left to the table.
    pub reason: String,
}

/// The support of one card and the fingerprint a deck-level hash is built from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportRecord {
    /// The oracle id.
    pub oracle_id: String,
    /// The computed state.
    pub support: Support,
    /// The unsupported clauses (`Partial`, or the reasons behind `Manual`).
    pub clauses: Vec<Clause>,
    /// Hash of the state, clauses, implementation and both oracle fingerprints.
    pub fingerprint: String,
}

/// The primitives the running engine implements, named as [`used_primitives`] names them
/// (`Effect.DealDamage`, `Keyword.Flying`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Caps {
    everything: bool,
    names: BTreeSet<String>,
}

impl Caps {
    /// An engine that runs nothing yet.
    pub fn none() -> Caps {
        Caps::default()
    }

    /// An engine that runs every primitive (tests, and the day the last stub is filled).
    pub fn everything() -> Caps {
        Caps {
            everything: true,
            names: BTreeSet::new(),
        }
    }

    /// An engine that runs exactly these primitives.
    pub fn from_names<I, S>(names: I) -> Caps
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Caps {
            everything: false,
            names: names.into_iter().map(Into::into).collect(),
        }
    }

    /// Whether the engine runs the primitive called `name`.
    pub fn runs(&self, name: &str) -> bool {
        self.everything || self.names.contains(name)
    }

    /// The primitives of `used` the engine does not run.
    pub fn missing(&self, used: &BTreeSet<String>) -> Vec<String> {
        if self.everything {
            return Vec::new();
        }
        used.iter()
            .filter(|name| !self.names.contains(*name))
            .cloned()
            .collect()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn digest(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
        hasher.update([0]);
    }
    hex(&hasher.finalize()[..16])
}

/// Lines trimmed, whitespace collapsed and blank lines dropped, so layout changes are not errata.
fn normalized(text: &str) -> String {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The fingerprint of an oracle text: SHA-256 of the normalized text, first 16 bytes as hex.
pub fn oracle_fingerprint(text: &str) -> String {
    hex(&Sha256::digest(normalized(text).as_bytes())[..16])
}

/// The hash of a card's implementation: the compact JSON of every face's abilities, in serde's field order.
pub fn implementation_hash(card: &Card) -> String {
    let abilities: Vec<_> = card.faces.iter().map(|face| &face.abilities).collect();
    let json = serde_json::to_vec(&abilities).unwrap_or_default();
    digest(&[&json])
}

fn state_tag(state: Support) -> &'static str {
    match state {
        Support::Verified => "verified",
        Support::Partial => "partial",
        Support::Manual => "manual",
        Support::Stale => "stale",
        Support::Unplayable => "unplayable",
    }
}

fn fingerprint(
    oracle_id: &str,
    state: Support,
    clauses: &[Clause],
    card: Option<&Card>,
    current: &[String],
) -> String {
    let mut parts: Vec<String> = vec![oracle_id.to_owned(), state_tag(state).to_owned()];
    parts.extend(
        clauses
            .iter()
            .map(|c| format!("{}:{}:{}:{}", c.face, c.span.start, c.span.end, c.reason)),
    );
    if let Some(card) = card {
        parts.push(implementation_hash(card));
        parts.extend(card.faces.iter().map(|f| f.oracle_fingerprint.clone()));
    }
    parts.extend(current.iter().cloned());
    let slices: Vec<&[u8]> = parts.iter().map(String::as_bytes).collect();
    digest(&slices)
}

pub(crate) fn whole_text(text: &str) -> Span {
    Span {
        start: 0,
        end: u32::try_from(text.chars().count()).unwrap_or(u32::MAX),
    }
}

/// The clauses and state of a card whose text is unchanged: declared clauses plus what the engine lacks.
fn gate(card: &Card, printed: &PrintedCard, caps: &Caps) -> (Support, Vec<Clause>) {
    let mut clauses = Vec::new();
    let mut covered = 0usize;
    for (index, face) in card.faces.iter().enumerate() {
        let mut gaps = BTreeSet::new();
        for ability in &face.abilities {
            if let crate::ability::Ability::Manual { reason, span } = ability {
                clauses.push(Clause {
                    face: index,
                    span: *span,
                    reason: reason.clone(),
                });
                continue;
            }
            let missing = caps.missing(&used_primitives(ability));
            if missing.is_empty() {
                covered += 1;
            }
            gaps.extend(missing);
        }
        if !gaps.is_empty() {
            let text = printed
                .faces
                .get(index)
                .map_or("", |f| f.oracle_text.as_str());
            clauses.push(Clause {
                face: index,
                span: whole_text(text),
                reason: format!(
                    "the engine does not run {} yet",
                    gaps.into_iter().collect::<Vec<_>>().join(", ")
                ),
            });
        }
    }
    let state = if clauses.is_empty() {
        Support::Verified
    } else if covered == 0 {
        Support::Manual
    } else {
        Support::Partial
    };
    (state, clauses)
}

/// The support of one card: a pure function of its rules file (if any), its imported text and the engine's capabilities.
pub fn support(card: Option<&Card>, printed: &PrintedCard, caps: &Caps) -> SupportRecord {
    let current: Vec<String> = printed
        .faces
        .iter()
        .map(|face| oracle_fingerprint(&face.oracle_text))
        .collect();
    let (state, clauses) = match card.map(|c| (c, c.impl_status)) {
        None | Some((_, ImplStatus::Manual)) => (Support::Manual, Vec::new()),
        Some((_, ImplStatus::Unplayable)) => (Support::Unplayable, Vec::new()),
        Some((card, ImplStatus::Full | ImplStatus::Partial)) => {
            let drifted = card.faces.len() != printed.faces.len()
                || card.faces.iter().zip(&printed.faces).zip(&current).any(
                    |((rules, face), now)| {
                        rules.name != face.name || rules.oracle_fingerprint != *now
                    },
                );
            let gaps = printed_gaps(printed, caps);
            if drifted {
                (Support::Stale, Vec::new())
            } else if !gaps.is_empty() {
                // Characteristics the engine cannot hold are no ground for trusting the rules.
                (Support::Manual, gaps)
            } else {
                gate(card, printed, caps)
            }
        }
    };
    SupportRecord {
        oracle_id: printed.oracle_id.clone(),
        fingerprint: fingerprint(&printed.oracle_id, state, &clauses, card, &current),
        support: state,
        clauses,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: fingerprints ignore layout but not wording, and the support state follows the declared status, the oracle text and what the engine runs.
    use super::*;
    use rstest::rstest;

    use crate::ability::Ability;
    use crate::card::Face;
    use crate::effect::Effect;
    use crate::keyword::Keyword;
    use crate::refs::{Ref, TargetSpec, Targetable};
    use crate::value::Amount;

    const TEXT: &str = "Lightning Bolt deals 3 damage to any target.";

    fn bolt_ability() -> Ability {
        Ability::Spell {
            targets: vec![TargetSpec {
                what: Targetable::AnyTarget,
                bounds: crate::refs::Bounds::default(),
                divide: None,
            }],
            effects: vec![Effect::DealDamage {
                source: None,
                to: Ref::Target(0),
                amount: Amount::Fixed(3),
            }],
        }
    }

    fn card(status: ImplStatus, abilities: Vec<Ability>, text: &str) -> Card {
        Card {
            oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
            name: "Lightning Bolt".into(),
            impl_status: status,
            faces: vec![Face {
                name: "Lightning Bolt".into(),
                oracle_fingerprint: oracle_fingerprint(text),
                abilities,
            }],
            notes: None,
        }
    }

    fn printed(text: &str) -> PrintedCard {
        PrintedCard {
            oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
            name: "Lightning Bolt".into(),
            keywords: vec![],
            faces: vec![PrintedFace {
                name: "Lightning Bolt".into(),
                oracle_text: text.into(),
                ..PrintedFace::default()
            }],
        }
    }

    #[rstest]
    #[case::same("a b\nc", "a b\nc", true)]
    #[case::spacing("a   b \n c", "a b\nc", true)]
    #[case::blank_lines("a b\n\nc\n", "a b\nc", true)]
    #[case::wording("a b\nc", "a c\nb", false)]
    #[case::case("Draw", "draw", false)]
    fn fingerprints_ignore_layout_only(
        #[case] left: &str,
        #[case] right: &str,
        #[case] equal: bool,
    ) {
        assert_eq!(oracle_fingerprint(left) == oracle_fingerprint(right), equal);
        assert_eq!(oracle_fingerprint(left).len(), 32);
    }

    #[test]
    fn the_fingerprint_of_nothing_is_the_hash_of_the_empty_string() {
        assert_eq!(oracle_fingerprint(""), "e3b0c44298fc1c149afbf4c8996fb924");
    }

    #[rstest]
    #[case::runs(Caps::everything(), Support::Verified)]
    #[case::engine_has_nothing(Caps::none(), Support::Manual)]
    #[case::engine_has_it(Caps::from_names(["Ability.Spell", "Effect.DealDamage"]), Support::Verified)]
    fn the_engine_decides_between_verified_and_manual(
        #[case] caps: Caps,
        #[case] expected: Support,
    ) {
        let record = support(
            Some(&card(ImplStatus::Full, vec![bolt_ability()], TEXT)),
            &printed(TEXT),
            &caps,
        );
        assert_eq!(record.support, expected);
        assert_eq!(record.clauses.is_empty(), expected == Support::Verified);
    }

    #[test]
    fn a_gap_in_the_engine_is_a_clause_over_the_face_text() {
        let record = support(
            Some(&card(
                ImplStatus::Full,
                vec![bolt_ability(), Ability::Keyword(Keyword::Flying)],
                TEXT,
            )),
            &printed(TEXT),
            &Caps::from_names(["Keyword.Flying"]),
        );
        assert_eq!(record.support, Support::Partial);
        assert_eq!(record.clauses.len(), 1);
        assert_eq!(record.clauses[0].span, Span { start: 0, end: 44 });
        assert!(
            record.clauses[0].reason.contains("Effect.DealDamage"),
            "{:?}",
            record.clauses
        );
    }

    #[test]
    fn changed_text_makes_the_card_stale_and_changes_its_fingerprint() {
        let rules = card(ImplStatus::Full, vec![bolt_ability()], TEXT);
        let before = support(Some(&rules), &printed(TEXT), &Caps::everything());
        let after = support(
            Some(&rules),
            &printed("Lightning Bolt deals 4 damage to any target."),
            &Caps::everything(),
        );
        assert_eq!(after.support, Support::Stale);
        assert_ne!(before.fingerprint, after.fingerprint);
    }

    #[rstest]
    #[case::no_file(None, Support::Manual)]
    #[case::manual(Some(ImplStatus::Manual), Support::Manual)]
    #[case::unplayable(Some(ImplStatus::Unplayable), Support::Unplayable)]
    fn cards_without_rules_are_manual_and_declared_unplayable_stays(
        #[case] status: Option<ImplStatus>,
        #[case] expected: Support,
    ) {
        let rules = status.map(|s| card(s, vec![], TEXT));
        let record = support(rules.as_ref(), &printed(TEXT), &Caps::everything());
        assert_eq!(record.support, expected);
    }

    #[test]
    fn declared_clauses_make_a_partial_card_and_keep_their_spans() {
        let clause = Ability::Manual {
            reason: "no effect for this yet".into(),
            span: Span { start: 5, end: 12 },
        };
        let record = support(
            Some(&card(
                ImplStatus::Partial,
                vec![Ability::Keyword(Keyword::Flying), clause],
                TEXT,
            )),
            &printed(TEXT),
            &Caps::everything(),
        );
        assert_eq!(record.support, Support::Partial);
        assert_eq!(record.clauses[0].span, Span { start: 5, end: 12 });
    }

    #[test]
    fn the_fingerprint_follows_the_data_the_state_and_the_text() {
        let a = card(ImplStatus::Full, vec![bolt_ability()], TEXT);
        let mut b = a.clone();
        b.faces[0].abilities = vec![Ability::Keyword(Keyword::Flying)];
        let p = printed(TEXT);
        let caps = Caps::everything();
        let fa = support(Some(&a), &p, &caps).fingerprint;
        assert_eq!(fa, support(Some(&a), &p, &caps).fingerprint);
        assert_ne!(fa, support(Some(&b), &p, &caps).fingerprint);
        assert_ne!(fa, support(Some(&a), &p, &Caps::none()).fingerprint);
        assert_ne!(fa, support(None, &p, &caps).fingerprint);
    }
}
