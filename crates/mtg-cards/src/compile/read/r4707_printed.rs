//! The suffix "with a name originally printed in the Antiquities expansion" (`Pred::OriginallyPrintedIn`): Golgothian
//! Sylex, Apocalypse Chime, City in a Bottle.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::modify::{GameRule, Prohibited};
use crate::r4707_first_sets::FirstSet;
use crate::refs::{Pred, Ref, ZoneKind};

/// The words before the expansion's name.
const R4707_HEAD: &str = "with a name originally printed in the";

/// Reads the suffix.
pub(super) fn r4707_suffix(r: &mut Reader<'_>) -> Option<Pred> {
    r.need(R4707_HEAD)?;
    let set = FirstSet::ALL
        .into_iter()
        .find(|set| r.words(&set.title().to_lowercase()))?;
    r.need("expansion")?;
    Some(Pred::OriginallyPrintedIn(set))
}

/// The suffix, written back; none for another predicate.
pub(super) fn r4707_suffix_text(pred: &Pred) -> Option<String> {
    let Pred::OriginallyPrintedIn(set) = pred else {
        return None;
    };
    Some(format!("{R4707_HEAD} {} expansion", set.title()))
}

/// "Players can't cast spells or play lands with a name originally printed in the Arabian Nights expansion.": the ban on
/// casting and the ban on land plays of the same players over the same original names, as one sentence; empty for any
/// other two abilities.
pub(super) fn r4707_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::Rule {
            rule:
                GameRule::Cant {
                    who,
                    action:
                        Prohibited::Cast {
                            filter,
                            more_than: None,
                            unless: None,
                        },
                },
            condition: None,
            from: ZoneKind::Battlefield,
        },
        Ability::Rule {
            rule:
                GameRule::Cant {
                    who: players,
                    action: Prohibited::PlayLands(lands),
                },
            condition: None,
            from: ZoneKind::Battlefield,
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    let ([pred @ Pred::OriginallyPrintedIn(_)], true, Ref::EachPlayer) =
        (filter.0.as_slice(), filter == lands, who)
    else {
        return Vec::new();
    };
    if players != who {
        return Vec::new();
    }
    r4707_suffix_text(pred)
        .map(|suffix| format!("Players can't cast spells or play lands {suffix}."))
        .into_iter()
        .collect()
}
