//! Doubling (CR 701.10d): "Double target creature's power and toughness until end of turn." is a change of the target
//! by its own power and toughness, counted as the spell resolves. Read from the wording and written back.

use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable};
use crate::value::{Amount, StatKind};

/// "Double target creature's power until end of turn." and "... power and toughness ...": `+X/+0` or `+X/+Y`, where X
/// is its power and Y its toughness.
pub fn c9_fight_double(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.targets.is_empty().then_some(())?;
    r.need("double target creature's power")?;
    let toughness = r.words("and toughness");
    r.need("until end of turn")?;
    r.targets.push(TargetSpec {
        what: Targetable::Object(Selector(vec![Pred::Type(
            mtg_core::card::CardType::Creature,
        )])),
        bounds: Bounds::default(),
        divide: None,
    });
    let affected = Ref::Target(0);
    Some(vec![Effect::Continuous {
        mods: vec![Modification::ModifyPT {
            power: c9_fight_stat(&affected, StatKind::Power),
            toughness: if toughness {
                c9_fight_stat(&affected, StatKind::Toughness)
            } else {
                Amount::Fixed(0)
            },
        }],
        affected,
        duration: Duration::UntilEndOfTurn,
    }])
}

/// A characteristic of `of`.
fn c9_fight_stat(of: &Ref, stat: StatKind) -> Amount {
    Amount::Stat {
        of: of.clone(),
        stat,
    }
}

/// What a doubling changes, without the subject: "power and toughness" or "power"; `None` for any other change.
pub fn c9_fight_doubled(affected: &Ref, mods: &[Modification]) -> Option<&'static str> {
    let [Modification::ModifyPT { power, toughness }] = mods else {
        return None;
    };
    (*power == c9_fight_stat(affected, StatKind::Power)).then_some(())?;
    if *toughness == c9_fight_stat(affected, StatKind::Toughness) {
        Some("power and toughness")
    } else {
        (*toughness == Amount::Fixed(0)).then_some("power")
    }
}
