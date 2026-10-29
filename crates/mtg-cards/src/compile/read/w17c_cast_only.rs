//! A spell's casting restriction (CR 601.3): "Cast this spell only during combat.", "Cast this spell only during combat
//! on an opponent's turn."

use super::Reader;
use crate::ability::Ability;
use crate::value::Condition;

/// The moments a spell may be cast in, each with its wording after "only".
fn w17c_moments() -> [(Condition, &'static str); 2] {
    [
        (
            Condition::All(vec![
                Condition::InCombat,
                Condition::Not(Box::new(Condition::YourTurn)),
            ]),
            "during combat on an opponent's turn",
        ),
        (Condition::InCombat, "during combat"),
    ]
}

/// "Cast ~ only <moment>": the spell is cast only while the moment holds for its caster.
pub(super) fn w17c_cast_only(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("cast")?;
    r.source().then_some(())?;
    r.need("only")?;
    w17c_moments().into_iter().find_map(|(condition, words)| {
        r.attempt(|r| {
            r.need(words)?;
            Some(Ability::CastOnly {
                condition: condition.clone(),
            })
        })
    })
}

/// The sentence of a casting restriction the reader reads, without the period; empty for any other ability.
pub(super) fn w17c_cast_only_texts(ability: &Ability) -> Vec<String> {
    let Ability::CastOnly { condition } = ability else {
        return Vec::new();
    };
    w17c_moments()
        .into_iter()
        .filter(|(moment, _)| moment == condition)
        .map(|(_, words)| format!("cast this spell only {words}"))
        .collect()
}
