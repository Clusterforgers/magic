//! Tokens a spell creates after another step: "Return target creature to its owner's hand. You create X 1/1 blue Faerie
//! creature tokens with flying." The renderers open the sentence with "Then".

use crate::ability::Ability;
use crate::effect::Effect;

/// Adds the wordings of a token creation after another effect that begins "You create".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let after = matches!(
        ability.effects(),
        [first, .., Effect::CreateToken { .. }] if !matches!(first, Effect::CreateToken { .. })
    );
    if !after {
        return;
    }
    let (from, into) = (". Then create ", ". You create ");
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replacen(from, into, 1))
        .collect();
    texts.extend(said);
}
