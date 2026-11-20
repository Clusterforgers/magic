//! What causes a draw (CR 614.11): the spell or ability resolving as the card would be drawn. A draw while an activated
//! ability resolves is that ability's, of its kind (cycling, CR 702.29); its source, as it was, is `Ref::Dealer` of the
//! steps that replace the draw. A draw outside a resolution (the draw step, a cost) has no source.

use mtg_cards::ability::ActivationKind;
use mtg_cards::refs::Selector;

use crate::abilities::AbilityRef;
use crate::resolve::{ResolveError, Scope};
use crate::snapshot::Snapshot;
use crate::state::GameState;

/// The spell or ability that causes a draw now: its kind for an activated ability with one, and the source as it was
/// (the spell itself for a spell).
pub(crate) fn r111_draw_cause(state: &GameState) -> Option<(Option<ActivationKind>, Snapshot)> {
    let resolving = state.object(state.resolving()?.object)?;
    let Some(ability) = resolving.ability() else {
        return crate::snapshot::snapshot(state, resolving.id()).map(|shot| (None, shot));
    };
    let address = AbilityRef {
        object: ability.source().id,
        ability: ability.index(),
        source: ability.origin(),
    };
    let kind = crate::activation_kind::specific_as_it_was(
        state,
        address,
        ability.rules(),
        ability.snapshot(),
    );
    Some((kind, ability.snapshot().clone()))
}

/// Whether the draw now is caused by an ability of `kind` whose source, as it was, `what` selects.
pub(crate) fn r111_drawn_by(
    state: &GameState,
    scope: &Scope<'_>,
    kind: ActivationKind,
    what: &Selector,
) -> Result<bool, ResolveError> {
    let Some((Some(found), shot)) = r111_draw_cause(state) else {
        return Ok(false);
    };
    if found != kind {
        return Ok(false);
    }
    let mut seen = scope.clone();
    seen.snapshots.push(&shot);
    seen.as_was = true;
    seen.selects(what, shot.reference())
}
