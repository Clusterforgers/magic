//! A permanent you own as a target: "{T}, Pay 1 life: Destroy target permanent you own." The renderers have no wording
//! for the owner of a target that is no more than a permanent.

use super::super::variants;
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, TargetSpec, Targetable};

/// Adds the wordings of an ability whose first target is any permanent its controller owns.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some(TargetSpec {
        what: Targetable::Object(owned),
        ..
    }) = ability.targets().first()
    else {
        return;
    };
    if owned.0 != [Pred::OwnedBy(Ref::You)] {
        return;
    }
    let mut any = ability.clone();
    let (Ability::Spell { targets, .. }
    | Ability::Activated { targets, .. }
    | Ability::Triggered { targets, .. }) = &mut any
    else {
        return;
    };
    if let Some(first) = targets.first_mut() {
        first.what = Targetable::Object(Selector(Vec::new()));
    }
    let (from, into) = ("target permanent", "target permanent you own");
    texts.extend(
        variants(&any)
            .iter()
            .filter(|text| text.contains(from))
            .map(|text| text.replacen(from, into, 1)),
    );
}
