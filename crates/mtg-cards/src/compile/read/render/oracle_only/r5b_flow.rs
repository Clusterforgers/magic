//! Wordings of control flow offered on trust (r5b): the later resolutions of an ability counted this turn, said as
//! "If it's the second time" once the first sentence named "this ability has resolved this turn", and the spell mastery
//! count of "instant and/or sorcery cards".

use crate::ability::Ability;

const RESOLVED: &str = " time this ability has resolved this turn";
const EITHER: &str = " or more instant or sorcery cards in your graveyard";

/// Adds the shortened later resolutions and the "and/or" of spell mastery.
pub(super) fn add(_ability: &Ability, texts: &mut Vec<String>) {
    let mut found = Vec::new();
    for text in texts.iter() {
        let Some(first) = text.find(RESOLVED) else {
            continue;
        };
        let (head, tail) = text.split_at(first + RESOLVED.len());
        let mut shortened = tail.to_owned();
        for time in ["second", "third"] {
            for opening in ["If", "if"] {
                shortened = shortened.replace(
                    &format!("{opening} this is the {time}{RESOLVED}"),
                    &format!("{opening} it's the {time} time"),
                );
            }
        }
        if shortened != tail {
            found.push(format!("{head}{shortened}"));
        }
    }
    texts.extend(found);
    let either: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(EITHER))
        .map(|text| {
            text.replace(
                EITHER,
                " or more instant and/or sorcery cards in your graveyard",
            )
        })
        .collect();
    texts.extend(either);
}
