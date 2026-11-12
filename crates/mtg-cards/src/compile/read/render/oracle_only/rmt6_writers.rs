//! Wordings of the rmt6 pieces that only the checker needs: a static or rule that holds while you have an enduring story
//! ("As long as you have an enduring story, creatures you control get +1/+1."), a card that isn't legendary as a token,
//! a creature that blocks some creatures as though it had reach, and the rules of a booster draft.

use crate::ability::Ability;
use crate::modify::{CombatQuirk, DraftRule, GameRule, Modification};
use crate::refs::{Pred, Ref, Selector};
use crate::value::Condition;
use mtg_core::card::Supertype;

/// The condition's words after "As long as".
const STORY: &str = "you have an enduring story";

/// Adds the wordings of the rmt6 pieces.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    texts.extend(storied_texts(ability));
    texts.extend(token_texts(ability));
    texts.extend(reach_texts(ability));
    texts.extend(draft_texts(ability));
}

/// "As long as you have an enduring story, <the ability without the condition>": a static or a rule that holds only while
/// its controller has an enduring story.
fn storied_texts(ability: &Ability) -> Vec<String> {
    let plain = match ability {
        Ability::Static {
            affected,
            mods,
            condition: Some(Condition::EnduringStory(Ref::You)),
            defining: false,
            from,
        } => Ability::Static {
            affected: affected.clone(),
            mods: mods.clone(),
            condition: None,
            defining: false,
            from: *from,
        },
        Ability::Rule {
            rule,
            condition: Some(Condition::EnduringStory(Ref::You)),
            from,
        } => Ability::Rule {
            rule: rule.clone(),
            condition: None,
            from: *from,
        },
        _ => return Vec::new(),
    };
    let mut said = super::super::written_variants(&plain);
    said.extend(trigger_again_texts(&plain));
    said.iter()
        .map(|text| format!("As long as {STORY}, {}", lower_first(text)))
        .collect()
}

/// "If a triggered ability of a Dwarf you control triggers, that ability triggers an additional time.": a rule that
/// triggers again every ability of the creatures of one type the controller controls.
fn trigger_again_texts(ability: &Ability) -> Vec<String> {
    let Ability::Rule {
        rule:
            GameRule::TriggerAgain {
                cause: None,
                source: Some(Selector(source)),
            },
        condition: None,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let [Pred::Subtype(subtype), Pred::ControlledBy(Ref::You)] = source.as_slice() else {
        return Vec::new();
    };
    let article = if subtype.0.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    vec![format!(
        "If a triggered ability of {article} {} you control triggers, that ability triggers an additional time.",
        subtype.0
    )]
}

/// "~ isn't legendary if it's a token.": the card loses the legendary supertype while it is a token.
fn token_texts(ability: &Ability) -> Vec<String> {
    let Ability::Static {
        affected: Ref::This,
        mods,
        condition:
            Some(Condition::Matches {
                subject: Ref::This,
                selector,
            }),
        defining: false,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let legendary = [Modification::RemoveSupertype(Supertype::Legendary)];
    if mods.as_slice() != legendary || selector.0 != [Pred::Token] {
        return Vec::new();
    }
    vec!["~ isn't legendary if it's a token.".to_owned()]
}

/// "~ can block Dragons as though it had reach.": the card blocks the creatures the selector picks as though it had reach.
fn reach_texts(ability: &Ability) -> Vec<String> {
    let Ability::Rule {
        rule:
            GameRule::Combat {
                who: Ref::This,
                quirk: CombatQuirk::BlockAsThoughReach(Selector(attackers)),
            },
        condition: None,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let [Pred::Subtype(subtype)] = attackers.as_slice() else {
        return Vec::new();
    };
    vec![format!(
        "~ can block {} as though it had reach.",
        plural(&subtype.0)
    )]
}

/// The plural of a creature type as the cards print it in a group ("Dragons", "Elves").
fn plural(subtype: &str) -> String {
    match subtype {
        "Elf" => "Elves".to_owned(),
        "Dwarf" => "Dwarves".to_owned(),
        _ => format!("{subtype}s"),
    }
}

/// The sentences of a rule of the booster draft, which works only during a draft and so never in a game.
fn draft_texts(ability: &Ability) -> Vec<String> {
    let Ability::Rule {
        rule: GameRule::Draft(rule),
        condition: None,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let text = match rule {
        DraftRule::FaceUp => "Draft ~ face up.".to_owned(),
        DraftRule::AtRandom { picks } => format!(
            "As long as ~ is face up during the draft, you can't look at booster packs and must draft cards at random. \
             After you draft {} cards this way, turn ~ face down.",
            number(*picks)
        ),
        DraftRule::LookAtNextPick => {
            "Reveal ~ as you draft it. You may look at the next card drafted from this booster pack.".to_owned()
        }
        DraftRule::LastCardsPassed => {
            "Each player passes the last card from each booster pack to a player who drafted a card named ~.".to_owned()
        }
        DraftRule::TradeAfterDraft => "Immediately after the draft, you may reveal a card in your card pool. Each other \
                                        player may offer you one card in their card pool in exchange. You may accept any \
                                        one offer."
            .to_owned(),
        DraftRule::LookAtNextDrafted => "During the draft, you may turn ~ face down. If you do, look at the next card \
                                         drafted by a player of your choice."
            .to_owned(),
        DraftRule::LookAtAnyPack => "During the draft, you may turn ~ face down. If you do, look at any unopened \
                                     booster pack in the draft or any booster pack not being looked at by another player."
            .to_owned(),
        DraftRule::DraftWholePack => "Instead of drafting a card from a booster pack, you may draft each card in that \
                                      booster pack, one at a time. If you do, turn ~ face down and you can't draft cards \
                                      for the rest of this draft round."
            .to_owned(),
        DraftRule::DraftAdditional => "As you draft a card, you may draft an additional card from that booster pack. If \
                                       you do, put ~ into that booster pack."
            .to_owned(),
        DraftRule::DraftAdditionalPassNext => "As you draft a card, you may draft an additional card from that booster \
                                               pack. If you do, turn ~ face down, then pass the next booster pack without \
                                               drafting a card from it."
            .to_owned(),
        DraftRule::RemoveDrafted { face_up } => format!(
            "As you draft a card, you may remove it from the draft face {}.",
            if *face_up { "up" } else { "down" }
        ),
        DraftRule::NoteName => {
            "As you draft a card, you may reveal it, note its name, then turn ~ face down.".to_owned()
        }
        DraftRule::NoteCreatureTypes => "As you draft a creature card, you may reveal it, note its creature types, then \
                                         turn ~ face down."
            .to_owned(),
    };
    // The newer printings say "this card" where the older ones name the card.
    let this_card = text.replace('~', "this card");
    if this_card == text {
        vec![text]
    } else {
        vec![text, this_card]
    }
}

/// A count as the cards spell it.
fn number(count: u32) -> String {
    const WORDS: [&str; 11] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    usize::try_from(count)
        .ok()
        .and_then(|at| WORDS.get(at))
        .map_or_else(|| count.to_string(), |word| (*word).to_owned())
}

/// The text with its first letter in lower case.
fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}
