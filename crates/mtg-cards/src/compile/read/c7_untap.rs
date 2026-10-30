//! A tap that holds the permanents down through their controller's next untap step: "Tap target creature an opponent
//! controls. That creature doesn't untap during its controller's next untap step.", "Tap up to two target creatures.
//! They don't untap during their controller's next untap step." (CR 502.3); and a shield over the deaths of the
//! creatures an effect named: "If that creature would die this turn, exile it instead." (CR 614)

use mtg_core::card::CardType;

use super::effects::object;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// The ways a card names the one permanent the sentence before tapped: `(as the writer says it, as a card may)`.
const C7_UNTAP_ONE: [(&str, &str); 3] = [
    (
        ". It doesn't untap during its controller's next untap step",
        ". That creature doesn't untap during its controller's next untap step",
    ),
    (
        ". It doesn't untap during its controller's next untap step",
        ". That permanent doesn't untap during its controller's next untap step",
    ),
    (
        ". They don't untap during their controller's next untap step",
        ". Those creatures don't untap during their controller's next untap step",
    ),
];

/// The tail of the sentence for one target and for several.
pub(super) const C7_UNTAP_TAILS: [&str; 2] = [
    "It doesn't untap during its controller's next untap step",
    "They don't untap during their controller's next untap step",
];

/// "tap <what>. <it> doesn't untap during <its controller's> next untap step": the tap and the rule that holds the
/// same permanents down, which lasts through that step.
pub(super) fn c7_untap_held(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("tap")?;
    let what = object(r)?;
    matches!(what, Ref::Target(0)).then_some(())?;
    r.punct('.').then_some(())?;
    if r.targets.first()?.bounds.is_one() {
        (r.word("it") || r.words("that creature") || r.words("that permanent")).then_some(())?;
        r.need("doesn't untap during its controller's next untap step")?;
    } else {
        (r.word("they") || r.words("those creatures")).then_some(())?;
        r.need("don't untap during their controller's next untap step")?;
    }
    Some(vec![
        Effect::Tap {
            what: what.clone(),
            bind: None,
        },
        Effect::Continuous {
            affected: what,
            mods: vec![Modification::DoesntUntap],
            duration: Duration::NextUntapStep,
        },
    ])
}

/// The card's words for the one permanent held down ("That creature doesn't untap ...") beside the writer's "It".
pub fn c7_untap_wordings(texts: &[String]) -> Vec<String> {
    C7_UNTAP_ONE
        .iter()
        .flat_map(|&(written, card)| {
            texts
                .iter()
                .filter(move |text| text.contains(written))
                .map(move |text| text.replacen(written, card, 1))
        })
        .collect()
}

/// The creatures a death shield names, as a card says them: `(selector, the words)`.
fn c7_untap_dying() -> [(Selector, &'static str); 3] {
    [
        (Selector(vec![Pred::Is(Ref::Target(0))]), "that creature"),
        (
            Selector(vec![
                Pred::Type(CardType::Creature),
                Pred::DamagedBy(Ref::This),
            ]),
            "a creature dealt damage this way",
        ),
        (
            Selector(vec![Pred::DamagedBy(Ref::This)]),
            "a permanent dealt damage this way",
        ),
    ]
}

/// "If that creature would die this turn, exile it instead.", "If a creature dealt damage this way would die this
/// turn, exile it instead.": a shield over the deaths of the creatures the effect named, fixed as it begins (CR 614,
/// 611.2c). "That creature" is the one target of the ability.
pub(super) fn c7_untap_dies(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("if")?;
    let one_target = r.targets.len() == 1 && matches!(r.targets[0].what, Targetable::Object(_));
    let what = c7_untap_dying()
        .into_iter()
        .enumerate()
        .find_map(|(at, (what, words))| {
            ((at > 0 || one_target) && r.words(words)).then_some(what)
        })?;
    r.need("would die this turn")?;
    r.punct(',').then_some(())?;
    r.need("exile it instead")?;
    Some(vec![c7_untap_shield(what)])
}

/// The shield that exiles the objects the selector names instead of letting them die this turn.
fn c7_untap_shield(what: Selector) -> Effect {
    Effect::Shield {
        when: ReplaceWhen::ZoneChange {
            what,
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        },
        by: ReplaceBy::MoveTo(ZoneKind::Exile),
        duration: Duration::UntilEndOfTurn,
    }
}

/// The wording of a death shield `c7_untap_dies` reads, without a capital and a period.
pub fn c7_untap_dies_text(effect: &Effect) -> Option<String> {
    let (_, words) = c7_untap_dying()
        .into_iter()
        .find(|(what, _)| c7_untap_shield(what.clone()) == *effect)?;
    Some(format!("if {words} would die this turn, exile it instead"))
}
