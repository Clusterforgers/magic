//! "Target creature an opponent controls attacks during its controller's next combat phase if able." (Trench Behemoth):
//! a requirement to attack that lasts until that creature's controller's next combat phase ends
//! (`Duration::UntilItsControllersNextCombatEnds`).

use super::effects::object;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule};
use crate::refs::Ref;

/// "<target creature> attacks during its controller's next combat phase if able".
pub(super) fn r5502_attacks_next_combat(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let what = object(r)?;
    matches!(what, Ref::Target(_)).then_some(())?;
    r.need("attacks during its controller's next combat phase if able")?;
    Some(vec![Effect::Rule {
        rule: GameRule::MustAttack(what),
        duration: Duration::UntilItsControllersNextCombatEnds,
    }])
}
