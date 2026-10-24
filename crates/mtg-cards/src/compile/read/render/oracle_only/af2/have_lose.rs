//! A player who loses life because you choose: "Whenever a creature an opponent controls enters, you may have that
//! player lose 1 life." The renderers write the choice with the controller as the subject.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

const SUBJECT: &str = ", you may that permanent's controller loses ";
const HAVE: &str = ", you may have that player lose ";

/// Adds the wordings of a choice to make the controller of the object of the event lose life.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let [
        Effect::May {
            who: Ref::You,
            effects,
        },
    ] = ability.effects()
    else {
        return;
    };
    let controller = Ref::ControllerOf(Box::new(Ref::Triggering));
    if !matches!(effects.as_slice(), [Effect::LoseLife { who, .. }] if *who == controller) {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(SUBJECT))
        .map(|text| text.replacen(SUBJECT, HAVE, 1))
        .collect();
    texts.extend(said);
}
