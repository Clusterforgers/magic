//! What an Aura adds to the creature it enchants without taking its other types: "Enchanted creature loses all
//! abilities and is a Toy artifact creature with base power and toughness 0/2 in addition to its other types." The
//! changes are the loss of abilities, the card types and creature types added, then the size (layers 6, 4, 4, 7b).

use mtg_core::card::{CardType, Subtype};

use super::triggers::article;
use crate::modify::Modification;
use crate::value::Amount;

/// The words that end the sentence of an Aura that adds types.
pub(super) const IN_ADDITION: &str = "in addition to its other types";

/// The words of an Aura that adds types, as the reader found them.
pub(super) struct Form<'a> {
    /// It loses all abilities first.
    pub lose: bool,
    /// The creature types.
    pub kinds: &'a [Subtype],
    /// It is an artifact too.
    pub artifact: bool,
    /// It is a creature.
    pub creature: bool,
    /// The base power and toughness.
    pub size: (i32, i32),
}

/// The changes of "[loses all abilities and] is a Toy artifact creature with base power and toughness 0/2 in addition to
/// its other types"; none for a permanent that is no creature (a color would replace the others, and the reader leaves it out).
pub(super) fn mods(form: &Form<'_>) -> Option<Vec<Modification>> {
    let Form {
        lose,
        kinds,
        artifact,
        creature,
        size,
    } = *form;
    (creature && !kinds.is_empty()).then_some(())?;
    let mut mods = Vec::new();
    mods.extend(lose.then_some(Modification::LoseAbilities));
    mods.extend(artifact.then_some(Modification::AddType(CardType::Artifact)));
    mods.push(Modification::AddType(CardType::Creature));
    mods.extend(kinds.iter().cloned().map(Modification::AddSubtype));
    mods.push(Modification::SetPT {
        power: Amount::Fixed(size.0),
        toughness: Amount::Fixed(size.1),
    });
    Some(mods)
}

/// Every wording of the changes `mods` reads; empty for any other list.
pub(super) fn texts(list: &[Modification]) -> Vec<String> {
    let (lose, rest) = match list.split_first() {
        Some((Modification::LoseAbilities, rest)) => (true, rest),
        _ => (false, list),
    };
    let (artifact, rest) = match rest.split_first() {
        Some((Modification::AddType(CardType::Artifact), rest)) => (true, rest),
        _ => (false, rest),
    };
    let [
        Modification::AddType(CardType::Creature),
        kinds @ ..,
        Modification::SetPT {
            power: Amount::Fixed(power),
            toughness: Amount::Fixed(toughness),
        },
    ] = rest
    else {
        return Vec::new();
    };
    let names: Option<Vec<&str>> = kinds
        .iter()
        .map(|kind| match kind {
            Modification::AddSubtype(Subtype(name)) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    let Some(names) = names.filter(|names| !names.is_empty()) else {
        return Vec::new();
    };
    let noun = format!(
        "{}{} creature",
        names.join(" "),
        if artifact { " artifact" } else { "" }
    );
    let head = format!(
        "is {} {noun} with base power and toughness {power}/{toughness} {IN_ADDITION}",
        article(&noun)
    );
    if lose {
        vec![format!("loses all abilities and {head}")]
    } else {
        vec![head]
    }
}
