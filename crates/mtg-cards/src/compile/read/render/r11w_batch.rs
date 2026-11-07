//! Steps done together as one batch ("~ deals 2 damage to any target and 1 damage to another target", "Destroy target
//! artifact and target land"): the batch only makes them one event for the triggers that look at them, so they read
//! as the steps themselves. Damage to the target and to what it names: "~ deals 4 damage to target creature and 2
//! damage to that creature's controller", "... and 3 damage to itself", "... and each other creature with the same
//! controller".

use super::{
    Ability, Amount, Effect, Form, Pred, Ref, Selector, Targetable, Writer, ZoneKind, phrase_text,
    points_text,
};
use crate::trigger::ReplaceWhen;
use mtg_core::card::CardType;

/// The effects with each batch replaced by its steps, or `None` when there is no batch.
pub(super) fn flat(effects: &[Effect]) -> Option<Vec<Effect>> {
    effects
        .iter()
        .any(|effect| matches!(effect, Effect::Together { .. }))
        .then(|| {
            effects
                .iter()
                .flat_map(|effect| match effect {
                    Effect::Together { effects } => shields_last(effects),
                    other => vec![other.clone()],
                })
                .collect()
        })
}

/// Whether the step is a shield of a move (a death made an exile), which nothing in a batch of damage can meet.
fn moved_shield(step: &Effect) -> bool {
    matches!(
        step,
        Effect::Shield {
            when: ReplaceWhen::ZoneChange { .. },
            ..
        }
    )
}

/// The steps of a batch of damage, with a shield among them read after the damage: the batch deals all of it before
/// anything can die, so the shield holds for each death either way ("... and 2 damage to that creature's controller.
/// If that creature would die this turn, exile it instead.").
fn shields_last(steps: &[Effect]) -> Vec<Effect> {
    let damage_and_shields = steps
        .iter()
        .all(|step| matches!(step, Effect::DealDamage { .. }) || moved_shield(step));
    if !damage_and_shields {
        return steps.to_vec();
    }
    let (shields, damage): (Vec<Effect>, Vec<Effect>) =
        steps.iter().cloned().partition(moved_shield);
    damage.into_iter().chain(shields).collect()
}

/// The ability with each batch among its effects replaced by its steps, so the writers of whole abilities see them.
pub(super) fn flat_ability(ability: &Ability) -> Option<Ability> {
    let steps = flat(ability.effects())?;
    let mut plain = ability.clone();
    match &mut plain {
        Ability::Spell { effects, .. }
        | Ability::Activated { effects, .. }
        | Ability::Loyalty { effects, .. }
        | Ability::Triggered { effects, .. } => *effects = steps,
        _ => return None,
    }
    Some(plain)
}

/// The other creatures a selector names beside the one target creature, as the oracle words them.
fn other_creatures(preds: &[Pred]) -> Option<&'static str> {
    let target = Ref::Target(0);
    let creature = Pred::Type(CardType::Creature);
    let other = Pred::Not(Box::new(Pred::Is(target.clone())));
    let [kind, not, which] = preds else {
        return None;
    };
    (*kind == creature && *not == other).then_some(())?;
    match which {
        Pred::SharesSubtype(of) if *of == target => {
            Some("each other creature that shares a creature type with it")
        }
        Pred::ControlledBy(Ref::ControllerOf(of)) if **of == target => {
            Some("each other creature with the same controller")
        }
        _ => None,
    }
}

impl Writer<'_> {
    /// Damage to the one target creature (or any target) and to its controller, to the source itself or to the other
    /// creatures it names, in one sentence.
    pub(super) fn r11w_damage_pair(&mut self, group: &[Effect]) -> Option<String> {
        let [
            Effect::DealDamage {
                source: None,
                to: Ref::Target(0),
                amount: first,
                ..
            },
            Effect::DealDamage {
                source: None,
                to: second,
                amount,
                ..
            },
        ] = group
        else {
            return None;
        };
        (self.targets.len() == 1 && self.single(&Ref::Target(0))).then_some(())?;
        let creature = self.target_is(|what| {
            matches!(what, Targetable::Object(Selector(preds)) if preds.first() == Some(&Pred::Type(CardType::Creature)))
        });
        let other = match second {
            Ref::This => "itself",
            Ref::ControllerOf(of) if **of == Ref::Target(0) && creature => {
                "that creature's controller"
            }
            Ref::Each(Selector(preds)) if creature => other_creatures(preds)?,
            _ => return None,
        };
        let dealt = points_text(first)?;
        let again = points_text(amount)?;
        let target = self.name_target()?;
        // "deals 3 damage to target creature and each other creature ...": one amount for both.
        if first == amount && matches!(second, Ref::Each(_)) {
            return Some(format!("~ deals {dealt} damage to {target} and {other}"));
        }
        Some(format!(
            "~ deals {dealt} damage to {target} and {again} damage to {other}"
        ))
    }
}

impl Writer<'_> {
    /// "Destroy any number of target creatures. For each creature destroyed this way, its controller creates a 4/4 red
    /// Dragon creature token with flying.": the removal and a token for the last controller of each object it removed,
    /// as sentences, and the two effects they cover.
    pub(super) fn r11w_removed_makes(
        &mut self,
        effects: &[Effect],
    ) -> Option<(Vec<String>, usize)> {
        let [
            removal,
            Effect::RepeatEach {
                over: Ref::Bound(removed),
                bind,
                effects: made,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let (plain, verb, what) = match removal {
            Effect::Destroy {
                what,
                regenerate,
                bind: Some(name),
            } if name == removed => (
                Effect::Destroy {
                    what: what.clone(),
                    regenerate: *regenerate,
                    bind: None,
                },
                "destroyed",
                what,
            ),
            Effect::Exile {
                what,
                bind: Some(name),
                duration: None,
            } if name == removed => (
                Effect::Exile {
                    what: what.clone(),
                    bind: None,
                    duration: None,
                },
                "exiled",
                what,
            ),
            _ => return None,
        };
        let [
            token @ Effect::CreateToken {
                who, bind: None, ..
            },
        ] = made.as_slice()
        else {
            return None;
        };
        let last = Ref::ControllerOf(Box::new(Ref::Departed(Box::new(Ref::Bound(bind.clone())))));
        (*who == last).then_some(())?;
        let creatures = match what {
            Ref::Target(0) => self.target_is(|kind| {
                matches!(kind, Targetable::Object(Selector(preds)) if preds.contains(&Pred::Type(CardType::Creature)))
            }),
            Ref::Each(Selector(preds)) => preds.contains(&Pred::Type(CardType::Creature)),
            _ => return None,
        };
        let kind = if creatures { "creature" } else { "permanent" };
        let first = self.effect(&plain)?;
        let makes = super::create::text(token, self.per)?;
        Some((
            vec![
                first,
                format!("for each {kind} {verb} this way, its controller {makes}"),
            ],
            2,
        ))
    }
}

/// "and 1 damage to any other target" beside "another target" when the second target is any target but the first.
pub(super) fn add_any_other(ability: &Ability, texts: &mut Vec<String>) {
    const ANOTHER: &str = " damage to another target";
    let [_, second, ..] = ability.targets() else {
        return;
    };
    let Targetable::Another { what, earlier } = &second.what else {
        return;
    };
    if **what != Targetable::AnyTarget || *earlier != [0] || !second.bounds.is_one() {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.matches(ANOTHER).count() == 1)
        .map(|text| text.replace(ANOTHER, " damage to any other target"))
        .collect();
    texts.extend(found);
}

/// "and 2 damage to its controller" beside "that creature's controller".
pub(super) fn add(texts: &mut Vec<String>) {
    const THAT: &str = " damage to that creature's controller";
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("damage to target creature and ") && text.contains(THAT))
        .map(|text| text.replace(THAT, " damage to its controller"))
        .collect();
    texts.extend(found);
}

/// What one pass over a player deals, after "equal to": "the number of lands that player controls", "twice the
/// number of ...", "the number of cards in that player's hand", "half that player's life total, rounded down".
fn per_player(amount: &Amount, each: &Ref) -> Option<String> {
    match amount {
        Amount::Times { by: 2, of } => Some(format!("twice {}", per_player(of, each)?)),
        Amount::Count(Selector(preds))
            if *preds == [Pred::OwnedBy(each.clone()), Pred::In(ZoneKind::Hand)] =>
        {
            Some("the number of cards in that player's hand".to_owned())
        }
        Amount::Count(Selector(preds)) => {
            let theirs = Pred::ControlledBy(each.clone());
            let rest: Vec<Pred> = preds
                .iter()
                .filter(|pred| **pred != theirs)
                .cloned()
                .collect();
            (rest.len() + 1 == preds.len()).then_some(())?;
            // Nothing else may name the player of the pass.
            (!serde_json::to_string(&rest).ok()?.contains("Bound")).then_some(())?;
            let phrase = phrase_text(&Selector(rest), Form::All)?;
            Some(format!("the number of {phrase} that player controls"))
        }
        Amount::Half { of, up: false } if **of == Amount::Life(each.clone()) => {
            Some("half that player's life total, rounded down".to_owned())
        }
        _ => None,
    }
}

/// "~ deals damage to each opponent equal to the number of Islands that player controls": one pass for each player
/// that deals that player an amount of their own.
pub(super) fn each_player_damage(effect: &Effect) -> Option<String> {
    let Effect::RepeatEach {
        over,
        bind,
        effects,
    } = effect
    else {
        return None;
    };
    let each = Ref::Bound(bind.clone());
    let [
        Effect::DealDamage {
            source: None,
            to,
            amount,
            ..
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    (*to == each).then_some(())?;
    let players = match over {
        Ref::EachPlayer => "each player",
        Ref::Opponents => "each opponent",
        _ => return None,
    };
    Some(format!(
        "~ deals damage to {players} equal to {}",
        per_player(amount, &each)?
    ))
}

/// "the number of lands they control" and "that opponent controls" beside "that player controls".
pub(super) fn add_players(texts: &mut Vec<String>) {
    const THAT: &str = " that player controls";
    let they: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(" damage to each player equal to ") && text.contains(THAT))
        .map(|text| text.replace(THAT, " they control"))
        .collect();
    let opponent: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(" damage to each opponent equal to ") && text.contains(THAT))
        .map(|text| text.replace(THAT, " that opponent controls"))
        .collect();
    // "creatures that player controls with flying": the keyword after the controller.
    let flying: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(" damage to each player equal to "))
        .filter_map(|text| {
            text.contains(" with flying that player controls").then(|| {
                text.replace(
                    " with flying that player controls",
                    " that player controls with flying",
                )
            })
        })
        .collect();
    texts.extend(they);
    texts.extend(opponent);
    texts.extend(flying);
}
