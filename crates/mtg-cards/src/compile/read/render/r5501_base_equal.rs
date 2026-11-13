//! A size taken from another creature: "~'s base power and toughness become equal to the power and toughness of target
//! creature other than ~ until the end of your next upkeep." (Halfdane): a `SetPT` of the card to the power and
//! toughness of its target, worked out as the effect begins.

use super::{Effect, Ref, Writer};
use crate::modify::{Duration, Modification};
use crate::value::{Amount, StatKind};

impl Writer<'_> {
    /// The sentence of an effect that sets the card's base power and toughness to those of an object.
    pub(super) fn r5501_base_equal(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Continuous {
            affected: Ref::This,
            mods,
            duration,
        } = effect
        else {
            return None;
        };
        let [
            Modification::SetPT {
                power:
                    Amount::Stat {
                        of: power_of,
                        stat: StatKind::Power,
                    },
                toughness:
                    Amount::Stat {
                        of: toughness_of,
                        stat: StatKind::Toughness,
                    },
            },
        ] = mods.as_slice()
        else {
            return None;
        };
        (power_of == toughness_of).then_some(())?;
        let until = match duration {
            Duration::UntilEndOfYourNextUpkeep => "until the end of your next upkeep",
            Duration::UntilEndOfTurn => "until end of turn",
            _ => return None,
        };
        let of = self.object(power_of)?;
        Some(format!(
            "~'s base power and toughness become equal to the power and toughness of {of} {until}"
        ))
    }
}
