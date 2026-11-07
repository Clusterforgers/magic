//! A creature put onto the battlefield that gains a color and a creature type for good: "Return target creature card
//! from your graveyard to the battlefield. That creature is a black Zombie in addition to its other colors and types."

use super::{Effect, Ref, Writer};
use crate::compile::read::selectors::color_name;
use crate::effect::Destination;
use crate::modify::{Duration, Modification};

impl Writer<'_> {
    /// The move onto the battlefield and the color and creature type the moved creature gains, as sentences, and the
    /// two effects they cover.
    pub(super) fn r11w_returned_as(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let [
            Effect::Move {
                what,
                to: to @ Destination::Battlefield { .. },
                bind: Some(moved),
            },
            Effect::Continuous {
                affected: Ref::Bound(affected),
                duration: Duration::Forever,
                mods,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        (affected == moved).then_some(())?;
        let [
            Modification::AddSubtype(subtype),
            Modification::AddColors(colors),
        ] = mods.as_slice()
        else {
            return None;
        };
        let [color] = colors.as_slice() else {
            return None;
        };
        let one = *what == Ref::Triggering || self.single(what);
        let several = matches!(what, Ref::Target(_)) && !self.at_most_one(what);
        (one || several).then_some(())?;
        let first = self.effect(&Effect::Move {
            what: what.clone(),
            to: to.clone(),
            bind: None,
        })?;
        let subject = if one {
            "that creature is"
        } else {
            "each of those creatures is"
        };
        Some((
            vec![
                first,
                format!(
                    "{subject} a {} {} in addition to its other colors and types",
                    color_name(*color),
                    subtype.0
                ),
            ],
            2,
        ))
    }
}
