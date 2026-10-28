//! Limits on a whole declaration: "No more than one creature can attack each combat." and "No more than two creatures
//! can block each combat." (Silent Arbiter, Dueling Grounds, Caverns of Despair), read into `GameRule::AttackLimit` and
//! `GameRule::BlockLimit`, and limits on the attacks at you or this planeswalker ("No more than two creatures can attack
//! you each combat.": Crawlspace, Judoon Enforcers, The Eternal Wanderer), read into `GameRule::AttackLimitAt`; all
//! written back.

use super::amounts::{count_word, number_word};
use super::tokens::Reader;
use crate::modify::GameRule;
use crate::refs::Ref;

/// "no more than one creature can attack each combat", "... can attack you each combat", "... can attack ~ each
/// combat", "no more than two creatures can block each combat".
pub fn w8a_limit(r: &mut Reader<'_>) -> Option<GameRule> {
    r.need("no more than")?;
    let most = number_word(r.peek_word()?).filter(|most| *most > 0)?;
    r.skip();
    let noun = if most == 1 { "creature" } else { "creatures" };
    r.need(noun)?;
    r.need("can")?;
    let rule = if r.word("attack") {
        if r.word("you") {
            GameRule::AttackLimitAt { most, at: Ref::You }
        } else if r.source() {
            GameRule::AttackLimitAt {
                most,
                at: Ref::This,
            }
        } else {
            GameRule::AttackLimit(most)
        }
    } else if r.word("block") {
        GameRule::BlockLimit(most)
    } else {
        return None;
    };
    r.need("each combat")?;
    Some(rule)
}

/// What `w8a_limit` reads, written back without a period; nothing for another rule.
pub fn w8a_limit_texts(rule: &GameRule) -> Vec<String> {
    let (most, verb) = match rule {
        GameRule::AttackLimit(most) => (*most, "attack"),
        GameRule::AttackLimitAt { most, at: Ref::You } => (*most, "attack you"),
        GameRule::AttackLimitAt {
            most,
            at: Ref::This,
        } => (*most, "attack ~"),
        GameRule::BlockLimit(most) => (*most, "block"),
        _ => return Vec::new(),
    };
    let Some(word) = count_word(most) else {
        return Vec::new();
    };
    let noun = if most == 1 { "creature" } else { "creatures" };
    vec![format!("No more than {word} {noun} can {verb} each combat")]
}
