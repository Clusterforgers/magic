//! Completing a proposal: what the imported card gives is filled in and the clauses nothing models become `Manual`.

use serde_json::Value;

use super::pairing::Paired;
use crate::ability::Ability;
use crate::card::{Card, ImplStatus};
use crate::load::{LoadError, parse_json};
use crate::support::{PrintedCard, oracle_fingerprint};

/// Why the checker leaves a clause to the table.
const OPEN_REASON: &str = "the proposed rules do not cover this clause";

/// Reads the JSON of a proposal as a `Card`: the oracle id, the name and the face names that are left out come from the
/// imported card, and the status is a placeholder that `finish` replaces.
pub(super) fn parse(text: &str, printed: &PrintedCard, file: &str) -> Result<Card, LoadError> {
    let problem = |message: String| LoadError {
        file: file.to_owned(),
        path: String::new(),
        message,
    };
    let mut value: Value =
        serde_json::from_str(text).map_err(|error| problem(format!("not JSON: {error}")))?;
    let Some(object) = value.as_object_mut() else {
        return Err(problem("a rules file is a JSON object".to_owned()));
    };
    object
        .entry("oracle_id")
        .or_insert_with(|| printed.oracle_id.clone().into());
    object
        .entry("name")
        .or_insert_with(|| printed.name.clone().into());
    object.insert("impl_status".to_owned(), "Full".into());
    let faces = object.entry("faces").or_insert_with(|| {
        printed
            .faces
            .iter()
            .map(|_| serde_json::json!({}))
            .collect::<Vec<_>>()
            .into()
    });
    for (face, shown) in faces
        .as_array_mut()
        .into_iter()
        .flatten()
        .zip(&printed.faces)
    {
        if let Some(face) = face.as_object_mut() {
            face.entry("name")
                .or_insert_with(|| shown.name.clone().into());
            face.insert("oracle_fingerprint".to_owned(), "".into());
        }
    }
    parse_json(file, &value.to_string())
}

/// The rules the checker judges: abilities in printed order, a `Manual` ability for each open clause, the status derived
/// from what is modelled and the fingerprints of the card's text.
pub(super) fn finish(mut card: Card, paired: &Paired, printed: &PrintedCard) -> Card {
    for (face, rules) in card.faces.iter_mut().enumerate() {
        let mut placed: Vec<(u32, Ability)> = std::mem::take(&mut rules.abilities)
            .into_iter()
            .enumerate()
            .map(|(at, ability)| {
                let key = paired
                    .places
                    .get(face)
                    .and_then(|places| places.get(at))
                    .copied()
                    .flatten()
                    .unwrap_or(u32::MAX);
                (key, ability)
            })
            .collect();
        placed.extend(paired.open(face).map(|span| {
            (
                span.start,
                Ability::Manual {
                    reason: OPEN_REASON.to_owned(),
                    span,
                },
            )
        }));
        placed.sort_by_key(|(key, _)| *key);
        rules.abilities = placed.into_iter().map(|(_, ability)| ability).collect();
    }
    let modelled = card
        .abilities()
        .any(|(_, ability)| !matches!(ability, Ability::Manual { .. }));
    let manual = card
        .abilities()
        .any(|(_, ability)| matches!(ability, Ability::Manual { .. }));
    card.impl_status = match (modelled, manual) {
        (false, _) => ImplStatus::Manual,
        (true, true) => ImplStatus::Partial,
        (true, false) => ImplStatus::Full,
    };
    for (rules, shown) in card.faces.iter_mut().zip(&printed.faces) {
        if card.impl_status == ImplStatus::Manual {
            rules.abilities.clear();
            rules.oracle_fingerprint.clear();
        } else {
            rules.oracle_fingerprint = oracle_fingerprint(&shown.oracle_text);
        }
    }
    card
}
