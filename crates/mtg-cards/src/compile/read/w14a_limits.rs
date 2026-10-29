//! Limits on a declaration under a condition or for one combat: "As long as ~ is tapped, no more than one creature can
//! attack you each combat." (an `Ability::Rule` with its condition) and "each opponent can't block with more than one
//! creature this combat" (`GameRule::BlockLimitEach` until end of combat, Mirri, Weatherlight Duelist); all written back.

use super::amounts::{count_word, number_word};
use super::as_long_as;
use super::tokens::Reader;
use super::w8a_declaration_limits::{w8a_limit, w8a_limit_texts};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule};
use crate::refs::{Ref, ZoneKind};
use crate::value::Condition;

/// "As long as ~ is tapped, no more than one creature can attack you each combat": a limit while the condition holds.
pub fn w14a_limit_while(r: &mut Reader<'_>) -> Option<Ability> {
    let condition = as_long_as::read(r, None)?;
    r.punct(',').then_some(())?;
    let rule = w8a_limit(r)?;
    Some(Ability::Rule {
        rule,
        condition: Some(condition),
        from: ZoneKind::Battlefield,
    })
}

/// What `w14a_limit_while` reads, written back; nothing for another rule.
pub fn w14a_limit_while_texts(rule: &GameRule, condition: &Condition) -> Vec<String> {
    let limits = w8a_limit_texts(rule);
    as_long_as::texts(condition)
        .iter()
        .flat_map(|held| {
            limits
                .iter()
                .map(move |limit| format!("as long as {held}, {}", w14a_lower_first(limit)))
        })
        .collect()
}

/// The text with its first letter in lower case.
fn w14a_lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

/// The players a limit on blocks names, and their words.
const W14A_BLOCKERS: [(&str, Ref); 2] = [
    ("each opponent", Ref::Opponents),
    ("defending player", Ref::Defending),
];

/// "each opponent can't block with more than one creature this combat", "defending player can't block with more than
/// two creatures this combat".
pub fn w14a_block_limit_each(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let who = W14A_BLOCKERS
        .iter()
        .find_map(|(words, who)| r.words(words).then(|| who.clone()))?;
    r.need("can't block with more than")?;
    let most = number_word(r.peek_word()?).filter(|most| *most > 0)?;
    r.skip();
    r.need(if most == 1 { "creature" } else { "creatures" })?;
    r.need("this combat")?;
    Some(vec![Effect::Rule {
        rule: GameRule::BlockLimitEach { most, who },
        duration: Duration::UntilEndOfCombat,
    }])
}

/// What `w14a_block_limit_each` reads, written.
pub fn w14a_block_limit_each_text(effect: &Effect) -> Option<String> {
    let Effect::Rule {
        rule: GameRule::BlockLimitEach { most, who },
        duration: Duration::UntilEndOfCombat,
    } = effect
    else {
        return None;
    };
    let (words, _) = W14A_BLOCKERS.iter().find(|(_, named)| named == who)?;
    let noun = if *most == 1 { "creature" } else { "creatures" };
    Some(format!(
        "{words} can't block with more than {} {noun} this combat",
        count_word(*most)?
    ))
}
