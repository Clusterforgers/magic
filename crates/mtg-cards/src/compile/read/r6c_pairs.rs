//! Two statics of one object under one condition are one sentence: "This creature can't block and can't be blocked."
//! is the static with both changes, which the writers write.

use crate::ability::Ability;

/// The one static of two that affect the same objects under the same condition from the same zone; `None` otherwise.
pub fn merged(first: &Ability, second: &Ability) -> Option<Ability> {
    let (
        Ability::Static {
            affected,
            mods,
            condition,
            defining: false,
            from,
        },
        Ability::Static {
            affected: also,
            mods: more,
            condition: same,
            defining: false,
            from: there,
        },
    ) = (first, second)
    else {
        return None;
    };
    (affected == also && condition == same && from == there).then(|| Ability::Static {
        affected: affected.clone(),
        mods: mods.iter().chain(more).cloned().collect(),
        condition: condition.clone(),
        defining: false,
        from: *from,
    })
}
