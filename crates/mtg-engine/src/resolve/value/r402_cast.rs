//! The values r40-2 adds for the spell being cast: how many of its targets a selector matches ("for each creature it
//! targets"), read by a cost modifier as the cost is determined (CR 601.2f).

use mtg_cards::refs::Selector;

use super::{CastTargets, ResolveError, Scope};
use crate::choose::Target;

impl Scope<'_> {
    /// How many objects `selector` matches among the targets of the spell being cast, each counted once
    /// (`Amount::CastTargetsOf`): before its targets are announced, the most each specification may take among its legal
    /// targets that match, so the spell is judged at its cheapest.
    pub(super) fn r402_cast_targets_of(&self, selector: &Selector) -> Result<i64, ResolveError> {
        let (Some((spell, caster)), Some(CastTargets { choices, judging })) =
            (self.casting, self.cast_targets)
        else {
            return Err(ResolveError::Invalid("no spell is being cast".into()));
        };
        let count = match judging {
            None => {
                let mut found = 0usize;
                for object in choices.targeted_objects() {
                    if object.id != spell && self.selects(selector, object)? {
                        found += 1;
                    }
                }
                found
            }
            Some(specs) => {
                let mut found = 0usize;
                for spec in specs {
                    let mut matching = 0usize;
                    for target in crate::choose::legal_targets(self.state, spec, spell, caster) {
                        if let Target::Object(object) = target
                            && object.id != spell
                            && self.selects(selector, object)?
                        {
                            matching += 1;
                        }
                    }
                    let most = spec
                        .bounds
                        .max
                        .as_ref()
                        .and_then(|max| self.amount(max).ok())
                        .and_then(|max| usize::try_from(max).ok());
                    found += most.map_or(matching, |most| matching.min(most));
                }
                found
            }
        };
        Ok(i64::try_from(count).unwrap_or(i64::MAX))
    }
}
