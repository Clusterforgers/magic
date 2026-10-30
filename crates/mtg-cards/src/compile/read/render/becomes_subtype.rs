//! A creature taken for good that becomes one more kind: "Gain control of target creature. It becomes a Vampire in
//! addition to its other types."

use super::Writer;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;

impl Writer<'_> {
    /// The control of the one target and the subtype it gains, as sentences that no joint splits, and the number of
    /// effects they cover.
    pub(super) fn taken_and_typed(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Continuous {
                affected: Ref::Target(0),
                mods: take,
                duration: Duration::Forever,
            },
            Effect::Continuous {
                affected: Ref::Target(0),
                mods: typed,
                duration: Duration::Forever,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let [Modification::AddSubtype(subtype)] = typed.as_slice() else {
            return None;
        };
        let taken = self.control(&Ref::Target(0), take, &Duration::Forever)?;
        let article = if subtype.0.starts_with(['A', 'E', 'I', 'O', 'U']) {
            "an"
        } else {
            "a"
        };
        Some((
            format!(
                "{taken}. It becomes {article} {} in addition to its other types",
                subtype.0
            ),
            2,
        ))
    }
}
