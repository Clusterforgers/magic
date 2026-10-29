//! The permanents `w20c_taken` reads: "Untap target permanent and gain control of it until end of turn. It gains haste
//! until end of turn." and a taken creature that gets a size or a type after its untap ("Until end of turn, it has base
//! power and toughness 10/10 and gains trample, annihilator 2, and haste.").

use super::Writer;
use crate::compile::read::selectors::granted_text;
use crate::compile::read::triggers::article;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;
use crate::value::Amount;

impl Writer<'_> {
    /// "untap target permanent and gain control of it until end of turn", with the keywords it gains in a sentence of
    /// its own, and the number of effects it covers.
    pub(super) fn w20c_untapped_taken(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Untap { what: untapped },
            Effect::Continuous {
                affected,
                mods: take,
                duration: Duration::UntilEndOfTurn,
            },
            rest @ ..,
        ] = effects
        else {
            return None;
        };
        (untapped == affected && *affected == Ref::Target(0)).then_some(())?;
        let [Modification::SetController(Ref::You)] = take.as_slice() else {
            return None;
        };
        let head = format!(
            "untap {} and gain control of it until end of turn",
            self.w6e_taken(affected)?
        );
        if let [
            Effect::Continuous {
                affected: gaining,
                mods: gain,
                duration: Duration::UntilEndOfTurn,
            },
            ..,
        ] = rest
            && gaining == affected
            && !gain.is_empty()
            && gain
                .iter()
                .all(|part| matches!(part, Modification::AddKeyword(_)))
        {
            let keywords = granted_text(gain)?;
            return Some((format!("{head}. It gains {keywords} until end of turn"), 3));
        }
        Some((head, 2))
    }

    /// The control of the taken creature for the turn, its untap and "Until end of turn, it has base power and toughness
    /// 10/10 and gains haste", as sentences that no joint splits, and the number of effects they cover.
    pub(super) fn w20c_taken_changed(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Continuous {
                affected,
                mods: take,
                duration: Duration::UntilEndOfTurn,
            },
            Effect::Untap { what: untapped },
            Effect::Continuous {
                affected: changed,
                mods,
                duration: Duration::UntilEndOfTurn,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        (affected == untapped && affected == changed && *affected == Ref::Target(0))
            .then_some(())?;
        let [Modification::SetController(Ref::You)] = take.as_slice() else {
            return None;
        };
        (!mods
            .iter()
            .all(|part| matches!(part, Modification::AddKeyword(_))))
        .then_some(())?;
        let noun = self.w6e_noun()?;
        let taken = self.w6e_taken(affected)?;
        let parts = w20c_parts(mods)?;
        Some((
            format!(
                "gain control of {taken} until end of turn. Untap that {noun}. Until end of turn, it {}",
                parts.join(" and ")
            ),
            3,
        ))
    }
}

/// "has base power and toughness 10/10", "gains trample and haste", "becomes a Pirate in addition to its other types".
fn w20c_parts(mods: &[Modification]) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut rest = mods;
    while let Some((first, after)) = rest.split_first() {
        match first {
            Modification::SetPT {
                power: Amount::Fixed(power),
                toughness: Amount::Fixed(toughness),
            } if *power >= 0 && *toughness >= 0 => {
                parts.push(format!("has base power and toughness {power}/{toughness}"));
                rest = after;
            }
            Modification::AddKeyword(_) => {
                let run = rest
                    .iter()
                    .take_while(|part| matches!(part, Modification::AddKeyword(_)))
                    .count();
                parts.push(format!("gains {}", granted_text(&rest[..run])?));
                rest = &rest[run..];
            }
            Modification::AddSubtype(kind) => {
                parts.push(format!(
                    "becomes {} {} in addition to its other types",
                    article(&kind.0),
                    kind.0
                ));
                rest = after;
            }
            _ => return None,
        }
    }
    Some(parts)
}
