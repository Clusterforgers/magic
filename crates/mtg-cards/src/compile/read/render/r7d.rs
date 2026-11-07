//! r7d: a combat rule for the turn ("target creature must be blocked this turn if able", "~ can attack this turn as
//! though it didn't have defender"), and that rule joined to a change of the same creatures ("gets +3/+3 until end of
//! turn and must be blocked this turn if able"); a creature exiled instead of dying, a spell exiled instead of countered,
//! a target held down or changed while the source stays tapped, a target that blocks or can't block the source, and a
//! land given a basic land type for the turn.

use mtg_core::card::{CardType, Subtype};

use super::{Writer, is_a_type};
use crate::compile::read::triggers::article;
use crate::effect::Effect;
use crate::modify::{Blocking, CombatQuirk, Duration, GameRule, Modification};
use crate::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

use super::super::selectors::{Form, phrase_text};

impl Writer<'_> {
    /// The creatures a rule names and whether they are several: "~", "it" for the target named before, "target
    /// creature", "creatures you control".
    fn r7d_subject(&mut self, who: &Ref) -> Option<(String, bool)> {
        match who {
            Ref::This => Some(("~".to_owned(), false)),
            Ref::Target(0) if self.target_is(|what| matches!(what, Targetable::Object(_))) => {
                let plural = !self.at_most_one(who);
                if self.named {
                    (self.single(who)).then(|| ("it".to_owned(), false))
                } else {
                    Some((self.name_target()?, plural))
                }
            }
            Ref::Each(selector) => {
                let phrase = phrase_text(selector, Form::All)?;
                let all = selector.0.iter().all(is_a_type);
                Some((if all { format!("all {phrase}") } else { phrase }, true))
            }
            _ => None,
        }
    }

    /// "must be blocked this turn if able": what a combat rule for the turn says of its creatures, after them.
    fn r7d_predicate(quirk: &CombatQuirk, plural: bool) -> Option<String> {
        let (it, its) = if plural {
            ("they", "their")
        } else {
            ("it", "its")
        };
        Some(match quirk {
            CombatQuirk::MustBeBlocked(Blocking::AtLeastOne) => {
                "must be blocked this turn if able".to_owned()
            }
            CombatQuirk::AttackDespiteDefender => {
                format!("can attack this turn as though {it} didn't have defender")
            }
            CombatQuirk::AssignsNoCombatDamage => {
                let verb = if plural { "assign" } else { "assigns" };
                format!("{verb} no combat damage this turn")
            }
            CombatQuirk::AssignsAsUnblocked => {
                return Some(format!(
                    "assign {its} combat damage this turn as though {it} weren't blocked"
                ));
            }
            _ => return None,
        })
    }

    /// A combat rule that lasts the turn as a sentence without its period.
    pub(super) fn r7d_combat(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Rule {
            rule: GameRule::Combat { who, quirk },
            duration: Duration::UntilEndOfTurn,
        } = effect
        else {
            return None;
        };
        let (subject, plural) = self.r7d_subject(who)?;
        Some(match quirk {
            CombatQuirk::MustBeBlocked(Blocking::Every) => {
                format!("all creatures able to block {subject} this turn do so")
            }
            CombatQuirk::AssignsAsUnblocked => format!(
                "you may have {subject} {}",
                Self::r7d_predicate(quirk, plural)?
            ),
            _ => format!("{subject} {}", Self::r7d_predicate(quirk, plural)?),
        })
    }

    /// A change of creatures for the turn and a combat rule for the same creatures as one sentence: "target creature
    /// gets +3/+3 until end of turn and must be blocked this turn if able".
    pub(super) fn r7d_changed_and_ruled(&mut self, group: &[Effect]) -> Option<String> {
        let [
            Effect::Continuous {
                affected,
                mods,
                duration: duration @ Duration::UntilEndOfTurn,
            },
            Effect::Rule {
                rule: GameRule::Combat { who, quirk },
                duration: Duration::UntilEndOfTurn,
            },
        ] = group
        else {
            return None;
        };
        if affected != who || matches!(quirk, CombatQuirk::AssignsAsUnblocked) {
            return None;
        }
        let plural = match who {
            Ref::Each(_) => true,
            Ref::Target(_) => !self.at_most_one(who),
            _ => false,
        };
        let changed = self.pump(affected, mods, duration)?;
        Some(format!(
            "{changed} and {}",
            Self::r7d_predicate(quirk, plural)?
        ))
    }
}

/// The words of a change that lasts while the source stays tapped (Duration::WhileSourceTapped).
pub(in super::super) const WHILE_TAPPED: &str = "for as long as ~ remains tapped";

impl Writer<'_> {
    /// Whether the one target is a creature and only a creature: "that creature" names it.
    fn r7d_creature_target(&self) -> bool {
        self.target_is(|what| {
            matches!(what, Targetable::Object(selector)
                if selector.0.contains(&Pred::Type(CardType::Creature))
                    && !selector.0.iter().any(|pred| matches!(pred, Pred::AnyOf(_))))
        })
    }

    /// "if that creature would die this turn, exile it instead": a shield that exiles a creature that would die.
    pub(super) fn r7d_die_exiled(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Shield {
            when:
                ReplaceWhen::ZoneChange {
                    what: Selector(preds),
                    from: Some(ZoneKind::Battlefield),
                    to: Some(ZoneKind::Graveyard),
                    ..
                },
            by: ReplaceBy::MoveTo(ZoneKind::Exile),
            duration: Duration::UntilEndOfTurn,
        } = effect
        else {
            return None;
        };
        let creature = Pred::Type(CardType::Creature);
        let creature_or_planeswalker =
            Pred::AnyOf(vec![creature.clone(), Pred::Type(CardType::Planeswalker)]);
        let subject = match preds.as_slice() {
            [Pred::Is(Ref::Target(0)), kind]
                if *kind == creature
                    && self.named
                    && self.single(&Ref::Target(0))
                    && self.r7d_creature_target() =>
            {
                "that creature"
            }
            [Pred::Is(Ref::Target(0)), kind]
                if *kind == creature_or_planeswalker
                    && self.named
                    && self.single(&Ref::Target(0))
                    && self.target_is(|what| {
                        matches!(what, Targetable::Object(Selector(target))
                            if target.as_slice() == [creature_or_planeswalker.clone()])
                    }) =>
            {
                "that creature or planeswalker"
            }
            [kind] if *kind == creature => "a creature",
            [kind, Pred::ControlledBy(Ref::Opponents)] if *kind == creature => {
                "a creature an opponent controls"
            }
            _ => return None,
        };
        Some(format!(
            "if {subject} would die this turn, exile it instead"
        ))
    }

    /// "Counter target spell. If that spell is countered this way, exile it instead of putting it into its owner's
    /// graveyard": the shield on the spell and the counter, and the number of effects they take.
    pub(super) fn r7d_countered_exiled(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Shield {
                when:
                    ReplaceWhen::Countered {
                        what: Selector(preds),
                    },
                by: ReplaceBy::MoveTo(ZoneKind::Exile),
                duration: Duration::UntilEndOfTurn,
            },
            counter @ Effect::Counter {
                what: Ref::Target(0),
            },
            ..,
        ] = effects
        else {
            return None;
        };
        if preds.as_slice() != [Pred::Is(Ref::Target(0)), Pred::Spell]
            || !self.single(&Ref::Target(0))
        {
            return None;
        }
        let countered = self.effect(counter)?;
        Some((
            format!(
                "{countered}. If that spell is countered this way, exile it instead of putting it into its owner's graveyard"
            ),
            2,
        ))
    }

    /// "tap target creature. It doesn't untap during its controller's untap step for as long as ~ remains tapped", and the
    /// number of effects it takes.
    pub(super) fn r7d_tapped_held(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            tap @ Effect::Tap { what, bind: None },
            Effect::Continuous {
                affected,
                mods,
                duration: Duration::WhileSourceTapped,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        if affected != what
            || *what != Ref::Target(0)
            || mods.as_slice() != [Modification::DoesntUntap]
            || !self.single(what)
        {
            return None;
        }
        let tapped = self.effect(tap)?;
        Some((
            format!("{tapped}. It doesn't untap during its controller's untap step {WHILE_TAPPED}"),
            2,
        ))
    }

    /// "target creature can't block ~ this turn", "target creature blocks ~ this turn if able": what the target does
    /// about the source in this turn's combat.
    pub(super) fn r7d_blocks_source(&mut self, effect: &Effect) -> Option<String> {
        let (who, verb) = match effect {
            Effect::Continuous {
                affected,
                mods,
                duration: Duration::UntilEndOfTurn,
            } => {
                let [Modification::CanBlockOnly(Selector(only))] = mods.as_slice() else {
                    return None;
                };
                let [Pred::Not(not)] = only.as_slice() else {
                    return None;
                };
                (**not == Pred::Is(Ref::This)).then_some(())?;
                (affected, "can't block ~ this turn")
            }
            Effect::Rule {
                rule:
                    GameRule::MustBlockAttacker {
                        who,
                        attacker: Ref::This,
                    },
                duration: Duration::UntilEndOfTurn,
            } => (who, "blocks ~ this turn if able"),
            _ => return None,
        };
        (*who == Ref::Target(0) && self.single(who)).then_some(())?;
        let (subject, _) = self.r7d_subject(who)?;
        Some(format!("{subject} {verb}"))
    }

    /// "target land becomes an Island until end of turn": a land given one basic land type for the turn.
    pub(super) fn r7d_land_type(
        &mut self,
        affected: &Ref,
        mods: &[Modification],
        duration: &Duration,
    ) -> Option<String> {
        let [Modification::SetLandTypes(types)] = mods else {
            return None;
        };
        let [Subtype(name)] = types.as_slice() else {
            return None;
        };
        if *duration != Duration::UntilEndOfTurn
            || !["Plains", "Island", "Swamp", "Mountain", "Forest"].contains(&name.as_str())
            || !self.single(affected)
            || !self.target_is(|what| {
                matches!(what, Targetable::Object(selector)
                    if selector.0.first() == Some(&Pred::Type(CardType::Land)))
            })
        {
            return None;
        }
        let (subject, _) = self.r7d_subject(affected)?;
        Some(format!(
            "{subject} becomes {} {name} until end of turn",
            article(name)
        ))
    }
}

impl Writer<'_> {
    /// "target creature gets +2/-2 for as long as ~ remains tapped": a change that lasts while the source stays tapped.
    pub(super) fn r7d_while_tapped(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Continuous {
            affected,
            mods,
            duration: duration @ Duration::WhileSourceTapped,
        } = effect
        else {
            return None;
        };
        if !mods.iter().all(|modification| {
            matches!(
                modification,
                Modification::ModifyPT { .. } | Modification::AddKeyword(_)
            )
        }) {
            return None;
        }
        self.pump(affected, mods, duration)
    }
}
