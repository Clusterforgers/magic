//! Clash wordings: "clash with defending player" (the opponent named, not chosen) and a won clash that adds colorless
//! mana at the beginning of the next main phase: "Clash with an opponent. If you win, at the beginning of your next main
//! phase, you may add an amount of {C} equal to that spell's mana value."

use mtg_core::mana::ManaType;

use super::tokens::Reader;
use super::w9b_clash::W9B_CLASH;
use crate::effect::{Effect, Produce};
use crate::refs::{Ref, Targetable};
use crate::spend::ManaLasts;
use crate::trigger::{Step, TriggerPattern};
use crate::value::{Amount, StatKind};

/// The words of a clash with the defending player.
pub(super) const W15C_DEFENDING: &str = "clash with defending player";

/// The words of the delayed trigger of a won clash.
pub(super) const W15C_NEXT_MAIN: &str = "at the beginning of your next main phase";

/// "Clash with defending player.": a clash whose opponent is the defending player.
fn w15c_defending(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need(W15C_DEFENDING)?;
    Some(vec![w15c_clash_effect(Vec::new(), Some(Ref::Defending))])
}

/// A clash of the controller with `with`, or with an opponent they choose, that does `win` when won.
pub(super) fn w15c_clash_effect(win: Vec<Effect>, with: Option<Ref>) -> Effect {
    Effect::Clash {
        who: Ref::You,
        win,
        lose: Vec::new(),
        until: false,
        with,
    }
}

/// The delayed trigger of the controller's next main phase (the first or the second, whichever begins first) that may
/// add as much {C} as the mana value of the one target spell.
pub(super) fn w15c_next_main_mana() -> Effect {
    let main = |step| TriggerPattern::StepBegins {
        step,
        whose: Ref::You,
    };
    Effect::DelayedTrigger {
        when: TriggerPattern::AnyOf(vec![main(Step::Main1), main(Step::Main2)]),
        once: true,
        this_turn: false,
        effects: vec![Effect::May {
            who: Ref::You,
            effects: vec![Effect::AddMana {
                who: Ref::You,
                mana: Produce::Fixed(vec![ManaType::Colorless]),
                times: Amount::Stat {
                    of: Ref::Target(0),
                    stat: StatKind::ManaValue,
                },
                only: Vec::new(),
                lasts: ManaLasts::Step,
            }],
        }],
    }
}

/// "Clash with an opponent. If you win, at the beginning of your next main phase, you may add an amount of {C} equal
/// to that spell's mana value." after a sentence with one target spell.
fn w15c_next_main(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    (r.targets.len() == 1 && matches!(r.targets[0].what, Targetable::Object(_))).then_some(())?;
    r.need(W9B_CLASH)?;
    r.punct('.').then_some(())?;
    r.need("if you win")?;
    r.punct(',').then_some(())?;
    r.need(W15C_NEXT_MAIN)?;
    r.punct(',').then_some(())?;
    r.need("you may add an amount of")?;
    (r.symbol()? == "C").then_some(())?;
    r.need("equal to that spell's mana value")?;
    Some(vec![w15c_clash_effect(vec![w15c_next_main_mana()], None)])
}

/// The wordings that name the mana value limit of a free cast from the hand after the hand: "you may cast a spell from
/// your hand with mana value 8 or less without paying its mana cost", also as a sentence opened by "Then" after another.
pub(super) fn w15c_hand_first(texts: &[String]) -> Vec<String> {
    const LEAD: &str = "cast a spell with mana value ";
    const HAND: &str = " or less from your hand without paying";
    texts
        .iter()
        .filter_map(|text| {
            let (before, rest) = text.split_once(LEAD)?;
            let (number, after) = rest.split_once(HAND)?;
            Some(format!(
                "{before}cast a spell from your hand with mana value {number} or less without paying{after}"
            ))
        })
        .flat_map(|text| {
            let then = text
                .contains(". You may cast a spell from your hand")
                .then(|| text.replace(". You may cast", ". Then you may cast"));
            std::iter::once(text).chain(then)
        })
        .collect()
}

/// The clash wordings of this module, tried before `w11c_clash`.
pub(super) fn w15c_clash(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.attempt(w15c_defending)
        .or_else(|| r.attempt(w15c_next_main))
}
