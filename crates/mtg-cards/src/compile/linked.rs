//! The two linked triggers of Oblivion Ring, Journey to Nowhere, Fiend Hunter and the Wormfang cards: "When ~ enters,
//! exile target ... When ~ leaves the battlefield, return the exiled card to the battlefield under its owner's control."
//! stay two abilities (CR 607.2a). The second returns what the source exiled while it was on the battlefield, so a source
//! that leaves before its enter trigger resolves exiles the card for good (CR 610.3c and the rulings); "until ~ leaves" is
//! another card. The exile may be a target, a choice of the controller or of an opponent, or the cost that keeps the card
//! ("sacrifice it unless you exile a creature you control"), and the return names the battlefield, a hand or a graveyard.

use super::Pending;
use crate::ability::Ability;
use crate::cost::CostItem;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// The words of the return after "return the exiled card(s) ", each with where the cards go.
const RETURNS: [(&str, Return); 12] = [
    (
        "to the battlefield under its owner's control",
        Return::Battlefield,
    ),
    (
        "to the battlefield under their owner's control",
        Return::Battlefield,
    ),
    (
        "to the battlefield under their owners' control",
        Return::Battlefield,
    ),
    (
        "to the battlefield tapped under its owner's control",
        Return::Tapped,
    ),
    (
        "to the battlefield tapped under their owner's control",
        Return::Tapped,
    ),
    (
        "to the battlefield tapped under their owners' control",
        Return::Tapped,
    ),
    ("to its owner's hand", Return::Hand),
    ("to their owner's hand", Return::Hand),
    ("to their owners' hands", Return::Hand),
    ("to its owner's graveyard", Return::Graveyard),
    ("to their owner's graveyard", Return::Graveyard),
    ("to their owners' graveyards", Return::Graveyard),
];

/// Where the return trigger puts the exiled cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Return {
    /// The battlefield under their owners' control.
    Battlefield,
    /// The battlefield tapped under their owners' control.
    Tapped,
    /// Their owners' hands.
    Hand,
    /// Their owners' graveyards.
    Graveyard,
}

impl Return {
    /// The destination of the move.
    fn destination(self) -> Destination {
        let battlefield = |tapped| Destination::Battlefield {
            tapped,
            under: None,
            transformed: false,
            counters: Vec::new(),
        };
        match self {
            Self::Battlefield => battlefield(false),
            Self::Tapped => battlefield(true),
            Self::Hand => Destination::Hand,
            Self::Graveyard => Destination::Graveyard,
        }
    }
}

/// Where the clause "When ~ leaves the battlefield, return the exiled card ..." puts the cards, the card named by its
/// name or as "this enchantment"; none for another clause.
fn return_of(text: &str) -> Option<Return> {
    let rest = text.strip_prefix("When ")?;
    let (subject, rest) = rest.split_once(" leaves the battlefield, return the exiled ")?;
    (subject == "~" || subject.starts_with("this ")).then_some(())?;
    let rest = rest
        .strip_prefix("cards ")
        .or_else(|| rest.strip_prefix("card "))?;
    let words = rest.strip_suffix('.')?;
    RETURNS
        .iter()
        .find(|(said, _)| *said == words)
        .map(|(_, to)| *to)
}

/// Whether the ability triggers when the card itself enters the battlefield.
fn enters_itself(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::ZoneChange { what, from: None, to: Some(ZoneKind::Battlefield) },
            intervening_if: None,
            ..
        } if what.0 == [Pred::This]
    )
}

/// Whether the effects exile for good, by themselves, inside a `May`, or as the cost of a payment that keeps the card:
/// the exile the return trigger is linked to.
fn exiles_for_good(effects: &[Effect]) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::Exile {
            bind: None,
            duration: None,
            ..
        } => true,
        Effect::May { effects, .. } => exiles_for_good(effects),
        Effect::Pay { cost, .. } => cost
            .0
            .iter()
            .any(|item| matches!(item, CostItem::ExilePermanents { .. })),
        _ => false,
    })
}

/// The cards the source exiled while it was on the battlefield (`Pred::ExiledWith`).
fn exiled_with_this() -> Ref {
    Ref::Each(Selector(vec![
        Pred::In(ZoneKind::Exile),
        Pred::ExiledWith(Ref::This),
    ]))
}

/// "When ~ leaves the battlefield, return the exiled card to the battlefield under its owner's control.": the cards the
/// source exiled return to where the wording says.
fn return_trigger(to: Return) -> Ability {
    Ability::Triggered {
        when: TriggerPattern::ZoneChange {
            what: Selector(vec![Pred::This]),
            from: Some(ZoneKind::Battlefield),
            to: None,
        },
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![Effect::Move {
            what: exiled_with_this(),
            to: to.destination(),
            bind: None,
        }],
    }
}

/// The wordings of a return trigger this module reads, for the proposal checker.
pub(crate) fn return_wordings(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: Some(ZoneKind::Battlefield),
                to: None,
            },
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::Move {
            what: moved,
            to,
            bind: None,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    if what.0 != [Pred::This] || !targets.is_empty() || *moved != exiled_with_this() {
        return Vec::new();
    }
    let mut texts = Vec::new();
    for (said, kind) in &RETURNS {
        if kind.destination() == *to {
            for card in ["card", "cards"] {
                texts.push(format!(
                    "When ~ leaves the battlefield, return the exiled {card} {said}."
                ));
            }
        }
    }
    texts
}

/// Reads the trigger that returns the exiled card as the second of two linked abilities, when an enter trigger of the same
/// card before it exiles for good.
pub(super) fn exile_pair(pending: &mut [Pending]) {
    let Some((second, to)) = pending.iter().enumerate().find_map(|(at, clause)| {
        (clause.ability.is_none() && !clause.read)
            .then(|| return_of(&clause.text))
            .flatten()
            .map(|to| (at, to))
    }) else {
        return;
    };
    let linked = pending[..second]
        .iter()
        .filter_map(|clause| clause.ability.as_ref())
        .filter(|ability| enters_itself(ability))
        .any(|ability| exiles_for_good(ability.effects()));
    if linked {
        pending[second].ability = Some(return_trigger(to));
        pending[second].read = true;
    }
}
