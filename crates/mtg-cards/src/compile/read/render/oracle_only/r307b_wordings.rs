//! Wordings of pack r30-7b: the other players than the one a trigger is about ("each of that player's opponents may
//! draw a card"), control given to the controller of the source that dealt damage ("that source's controller gains
//! control of this creature") and a target destroyed unless its controller has the source deal damage equal to its
//! power ("unless its controller has ~ deal damage to them equal to his power"), "different kinds of counters", an Aura
//! attached to the player who attacks you, the life drain of an attack on the enchanted player, a creature made to
//! attack the damaged player each combat until the end of your next turn, and a card from your hand put onto the
//! battlefield tapped and attacking with a conversion at end of combat if you do.

use super::super::super::selectors::{Form, phrase_text};
use super::super::super::triggers::article;
use super::r288_wordings::r288_heads;
use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::effect::{Destination, Effect};
use crate::modify::{Duration, GameRule, Modification};
use crate::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use crate::trigger::{Step, TriggerPattern};
use crate::value::{Amount, Condition, StatKind};
use mtg_core::card::CardType;

/// Adds the wordings of the shapes this family writes.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for text in that_players_opponents(ability)
        .into_iter()
        .chain(source_controller_gains_control(ability))
        .chain(unless_damage_equal_to_power(ability))
        .chain(attach_to_attacker(ability))
        .chain(drain_enchanted_player(ability))
        .chain(attacks_that_player(ability))
        .chain(put_attacking_then_convert(ability))
    {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
    different_kinds(texts);
}

/// "the number of different kinds of counters": every kind counts once, as "the number of kinds of counters" says.
fn different_kinds(texts: &mut Vec<String>) {
    let more: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("the number of kinds of counters among"))
        .map(|text| {
            text.replace(
                "the number of kinds of counters among",
                "the number of different kinds of counters among",
            )
        })
        .collect();
    for text in more {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "Whenever a player attacks you, you may attach this Aura to that player.": the Aura moves to the attacking player.
fn attach_to_attacker(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when:
            TriggerPattern::PlayerAttacks {
                by: Ref::EachPlayer,
                at: Ref::You,
            },
        intervening_if: None,
        limit: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let attach = Effect::May {
        who: Ref::You,
        effects: vec![Effect::Attach {
            what: Ref::This,
            to: Ref::Triggering,
            bind: None,
        }],
    };
    if !targets.is_empty() || *effects != [attach] {
        return Vec::new();
    }
    vec!["Whenever a player attacks you, you may attach this Aura to that player.".to_owned()]
}

/// "Whenever you attack enchanted player, that player loses 2 life. You draw a card and gain 2 life.".
fn drain_enchanted_player(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when:
            TriggerPattern::PlayerAttacks {
                by: Ref::You,
                at: Ref::AttachedPlayer,
            },
        intervening_if: None,
        limit: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::LoseLife {
            who: Ref::AttachedPlayer,
            amount: Amount::Fixed(lost),
        },
        Effect::Draw {
            who: Ref::You,
            count: Amount::Fixed(1),
            bind: None,
        },
        Effect::GainLife {
            who: Ref::You,
            amount: Amount::Fixed(gained),
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    if !targets.is_empty() {
        return Vec::new();
    }
    vec![format!(
        "Whenever you attack enchanted player, that player loses {lost} life. You draw a card and gain {gained} life."
    )]
}

/// "Whenever ~ deals combat damage to a player, until the end of your next turn, target creature attacks that player
/// each combat if able.": the one target creature is paired with the player dealt the damage.
fn attacks_that_player(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when:
            TriggerPattern::DealsDamage {
                to: Some(Ref::EachPlayer),
                ..
            },
        targets,
        effects,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let creature = Selector(vec![Pred::Type(CardType::Creature)]);
    let [spec] = targets.as_slice() else {
        return Vec::new();
    };
    if spec.what != Targetable::Object(creature) || !spec.bounds.is_one() || spec.divide.is_some() {
        return Vec::new();
    }
    let rule = Effect::Rule {
        rule: GameRule::MustAttackPlayers {
            who: Ref::Target(0),
            players: Ref::Triggering,
        },
        duration: Duration::UntilEndOfYourNextTurn,
    };
    if *effects != [rule] {
        return Vec::new();
    }
    r288_heads(ability)
        .into_iter()
        .map(|head| {
            format!("{head} until the end of your next turn, target creature attacks that player each combat if able.")
        })
        .collect()
}

/// "Whenever ~ attacks, you may put an artifact creature card from your hand onto the battlefield tapped and attacking.
/// If you do, convert ~ at end of combat.": a card of the kinds chosen from the hand joins the attack, and the source
/// converts as the combat ends when one was put.
fn put_attacking_then_convert(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        targets, effects, ..
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::May {
            who: Ref::You,
            effects: steps,
        },
        Effect::If {
            condition: Condition::Performed,
            then,
            otherwise,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let convert = Effect::DelayedTrigger {
        when: TriggerPattern::StepBegins {
            step: Step::EndCombat,
            whose: Ref::EachPlayer,
        },
        once: true,
        this_turn: false,
        until: None,
        targets: Vec::new(),
        effects: vec![Effect::Transform { what: Ref::This }],
    };
    if !targets.is_empty() || !otherwise.is_empty() || *then != [convert] {
        return Vec::new();
    }
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count,
            bind,
            rest_as: None,
        },
        Effect::Move {
            what: Ref::Bound(moved),
            to:
                Destination::Battlefield {
                    tapped: true,
                    under: None,
                    transformed: false,
                    counters,
                },
            bind: None,
        },
        Effect::JoinAttack {
            what: Ref::Bound(joined),
        },
    ] = steps.as_slice()
    else {
        return Vec::new();
    };
    if moved != bind || joined != bind || !counters.is_empty() || !count.is_one() {
        return Vec::new();
    }
    let (kinds, place): (Vec<&Pred>, Vec<&Pred>) = from
        .0
        .iter()
        .partition(|pred| matches!(pred, Pred::Type(_) | Pred::Subtype(_)));
    if place.len() != 2
        || !place.contains(&&Pred::In(ZoneKind::Hand))
        || !place.contains(&&Pred::OwnedBy(Ref::You))
        || kinds.is_empty()
    {
        return Vec::new();
    }
    let Some(kind) = phrase_text(
        &Selector(kinds.into_iter().cloned().collect()),
        Form::Subject,
    ) else {
        return Vec::new();
    };
    let said = format!("{} {kind} card", article(&kind));
    r288_heads(ability)
        .into_iter()
        .map(|head| {
            format!(
                "{head} you may put {said} from your hand onto the battlefield tapped and attacking. If you do, convert ~ at end of combat."
            )
        })
        .collect()
}

/// Every player other than the one the event is about ("each of that player's opponents"): with no teams, a player's
/// opponents are all the other players (CR 102.2).
fn others_of_triggering(over: &Ref) -> bool {
    matches!(over, Ref::PlayersWhere { who, test }
        if **who == Ref::EachPlayer
            && **test == Condition::Not(Box::new(Condition::Among(Ref::Triggering))))
}

/// "Whenever a player casts a noncreature spell, each of that player's opponents may draw a card.": a repeat over the
/// other players than the one the event is about, worded as its twin over "each opponent" under a head about a player.
fn that_players_opponents(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when,
        intervening_if,
        limit,
        from,
        targets,
        effects,
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::RepeatEach {
            over,
            bind,
            effects: each,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    if !others_of_triggering(over)
        || !matches!(
            when,
            TriggerPattern::SpellCast {
                by: Ref::EachPlayer,
                ..
            }
        )
    {
        return Vec::new();
    }
    let twin = Ability::Triggered {
        when: when.clone(),
        intervening_if: intervening_if.clone(),
        limit: *limit,
        from: *from,
        targets: targets.clone(),
        effects: vec![Effect::RepeatEach {
            over: Ref::Opponents,
            bind: bind.clone(),
            effects: each.clone(),
        }],
    };
    crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.starts_with("Whenever a player "))
        .filter_map(|text| {
            let (head, rest) = text.split_once(", each opponent ")?;
            Some(format!("{head}, each of that player's opponents {rest}"))
        })
        .collect()
}

/// "Whenever a source deals damage to this creature, that source's controller gains control of this creature.": the
/// controller of the source that dealt the damage gets the creature for good.
fn source_controller_gains_control(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when:
            TriggerPattern::DealsDamage {
                source,
                to: Some(Ref::This),
                combat: None,
            },
        targets,
        effects,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let gained = Effect::Continuous {
        affected: Ref::This,
        mods: vec![Modification::SetController(Ref::ControllerOf(Box::new(
            Ref::Dealer,
        )))],
        duration: Duration::Forever,
    };
    if !source.is_empty() || !targets.is_empty() || *effects != [gained] {
        return Vec::new();
    }
    r288_heads(ability)
        .into_iter()
        .map(|head| format!("{head} that source's controller gains control of ~."))
        .collect()
}

/// "Whenever ~ attacks, destroy target permanent unless its controller has ~ deal damage to them equal to his power.":
/// the controller of the one target may take damage from the source equal to its power, else the target is destroyed.
fn unless_damage_equal_to_power(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        targets, effects, ..
    } = ability
    else {
        return Vec::new();
    };
    let [spec] = targets.as_slice() else {
        return Vec::new();
    };
    if spec.what != Targetable::Object(Selector(Vec::<Pred>::new()))
        || !spec.bounds.is_one()
        || spec.divide.is_some()
    {
        return Vec::new();
    }
    let power = Amount::Stat {
        of: Ref::This,
        stat: StatKind::Power,
    };
    let expected = Effect::Pay {
        who: Ref::ControllerOf(Box::new(Ref::Target(0))),
        cost: Cost(vec![CostItem::DamageToYou(power)]),
        times: Amount::Fixed(1),
        paid: Vec::new(),
        unpaid: vec![Effect::Destroy {
            what: Ref::Target(0),
            regenerate: true,
            bind: None,
        }],
    };
    if *effects != [expected] {
        return Vec::new();
    }
    let mut texts = Vec::new();
    for head in r288_heads(ability) {
        for pronoun in ["its", "his", "her"] {
            texts.push(format!(
                "{head} destroy target permanent unless its controller has ~ deal damage to them equal to {pronoun} power."
            ));
        }
    }
    texts
}
