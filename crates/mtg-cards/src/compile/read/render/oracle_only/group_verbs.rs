//! A group that regenerates or gains a keyword, worded the older way: "Regenerate each creature you control." and "All
//! Zombies gain menace until end of turn." The renderers write "all creatures" and put the duration first.

use super::super::super::subtypes::plural_subtype;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::Modification;
use crate::refs::Ref;

/// Adds the wordings of a regeneration of a group and of a keyword given to the members of a kind.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut effects: Vec<&Effect> = Vec::new();
    for effect in ability.effects() {
        match effect {
            Effect::Modal { modes, .. } => {
                effects.extend(modes.iter().flat_map(|mode| mode.effects.iter()));
            }
            other => effects.push(other),
        }
    }
    let regenerates = effects
        .iter()
        .any(|effect| matches!(effect, Effect::Regenerate { .. }));
    let gives = effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::Continuous { affected: Ref::Each(_), mods, .. }
                if mods.iter().all(|m| matches!(m, Modification::AddKeyword(_)))
        )
    });
    let mut said = Vec::new();
    for text in texts.iter() {
        if regenerates && text.contains("Regenerate all creatures you control") {
            said.push(text.replacen(
                "Regenerate all creatures you control",
                "Regenerate each creature you control",
                1,
            ));
        }
        if gives && let Some(all) = all_gain(text) {
            said.push(all);
        }
    }
    texts.extend(said);
}

/// "Until end of turn, each Zombie gains menace" as "All Zombies gain menace until end of turn".
fn all_gain(text: &str) -> Option<String> {
    let (head, rest) = text.split_once("Until end of turn, each ")?;
    let (kind, rest) = rest.split_once(" gains ")?;
    if kind.contains(' ') {
        return None;
    }
    let (keyword, tail) = rest.split_once('.')?;
    Some(format!(
        "{head}All {} gain {keyword} until end of turn.{tail}",
        plural_subtype(kind)
    ))
}
