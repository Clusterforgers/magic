//! The renderer: rules data back to the oracle wording the readers read, so a reading can be
//! checked against the text it came from. Every effect is a sentence of its own; the readers take
//! ", then" between two steps as the same sequence.

use super::amounts::{cards_text, count_word, points_text, stat_name};
use super::create;
use super::selectors::{Form, keyword_text, phrase_text, spec_text};
use super::triggers::{condition_text, pattern_text};
use crate::ability::{Ability, Timing};
use crate::cost::{Cost, CostItem};
use crate::effect::{Destination, Effect, Produce};
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, TargetSpec, Targetable, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Condition, CounterKind};

/// The oracle wording of an ability the readers produce; `None` for any other ability.
pub fn ability(ability: &Ability) -> Option<String> {
    match ability {
        Ability::Spell { targets, effects } => spell(targets, effects),
        Ability::Triggered {
            when,
            intervening_if,
            limit: None,
            from: ZoneKind::Battlefield,
            targets,
            effects,
        } => triggered(when, intervening_if.as_ref(), targets, effects),
        Ability::Activated {
            cost,
            timing: Timing::Instant,
            limit: None,
            from: ZoneKind::Battlefield,
            targets,
            effects,
        } if targets.is_empty() => mana_ability(cost, effects),
        _ => None,
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The sentences of an ability, one for each effect, without their first capital and period.
fn sentences(targets: &[TargetSpec], effects: &[Effect]) -> Option<Vec<String>> {
    if targets.len() > 1 || effects.is_empty() {
        return None;
    }
    let mut writer = Writer {
        targets,
        named: false,
    };
    let mut sentences: Vec<String> = Vec::new();
    for effect in effects {
        sentences.push(writer.effect(effect)?);
        if let Effect::Destroy {
            what,
            regenerate: false,
        } = effect
        {
            let pronoun = if writer.single(what) { "it" } else { "they" };
            sentences.push(format!("{pronoun} can't be regenerated"));
        }
    }
    (targets.is_empty() || writer.named).then_some(sentences)
}

/// The sentences of a spell, one for each effect.
fn spell(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let closed: Vec<String> = sentences(targets, effects)?
        .iter()
        .map(|sentence| format!("{}.", capitalize(sentence)))
        .collect();
    Some(closed.join(" "))
}

/// The event, an intervening "if" and the sentences of a triggered ability.
fn triggered(
    when: &TriggerPattern,
    condition: Option<&Condition>,
    targets: &[TargetSpec],
    effects: &[Effect],
) -> Option<String> {
    let mut parts = vec![format!("{},", pattern_text(when)?)];
    if let Some(condition) = condition {
        parts.push(format!("if {},", condition_text(condition)?));
    }
    for (at, sentence) in sentences(targets, effects)?.iter().enumerate() {
        parts.push(if at == 0 {
            format!("{sentence}.")
        } else {
            format!("{}.", capitalize(sentence))
        });
    }
    Some(parts.join(" "))
}

/// Whether the effect is written as a verb with "you" understood ("draw a card"), which "you may" can precede.
fn is_imperative(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::Scry { who: Ref::You, .. }
            | Effect::Draw { who: Ref::You, .. }
            | Effect::Mill { who: Ref::You, .. }
            | Effect::Discard { who: Ref::You, .. }
            | Effect::Destroy {
                regenerate: true,
                ..
            }
            | Effect::Exile { .. }
            | Effect::Counter { .. }
            | Effect::Move { .. }
            | Effect::CreateToken { who: Ref::You, .. }
            | Effect::PutCounters { .. }
    )
}

/// `{T}: Add {G}.`
fn mana_ability(cost: &Cost, effects: &[Effect]) -> Option<String> {
    if cost.0.as_slice() != [CostItem::Tap] {
        return None;
    }
    let [
        Effect::AddMana {
            who: Ref::You,
            mana,
            times,
        },
    ] = effects
    else {
        return None;
    };
    if !times.is_one() {
        return None;
    }
    let added = match mana {
        Produce::Fixed(list) if !list.is_empty() => {
            let mut symbols = String::new();
            for mana in list {
                symbols.push('{');
                symbols.push(mana.symbol());
                symbols.push('}');
            }
            symbols
        }
        Produce::AnyColor(1) => "one mana of any color".to_owned(),
        _ => return None,
    };
    Some(format!("{{T}}: Add {added}."))
}

/// "+2", "-3", "+X": a change of power or toughness.
fn signed_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Fixed(number) if *number >= 0 => Some(format!("+{number}")),
        Amount::Fixed(number) => Some(number.to_string()),
        Amount::X => Some("+X".to_owned()),
        _ => None,
    }
}

fn at_random(random: bool) -> &'static str {
    if random { " at random" } else { "" }
}

/// Writes effects in order, remembering whether the clause's target has been named yet.
struct Writer<'a> {
    targets: &'a [TargetSpec],
    named: bool,
}

impl Writer<'_> {
    /// The target, named in full the first time and never again.
    fn name_target(&mut self) -> Option<String> {
        if self.named {
            return None;
        }
        self.named = true;
        spec_text(self.targets.first()?)
    }

    /// Whether the reference is the one target and it is a single object.
    fn single(&self, what: &Ref) -> bool {
        matches!(what, Ref::Target(_))
            && self
                .targets
                .first()
                .is_some_and(|spec| spec.bounds.is_one())
    }

    /// Whether the target is of this kind.
    fn target_is(&self, wanted: impl Fn(&Targetable) -> bool) -> bool {
        self.targets.first().is_some_and(|spec| wanted(&spec.what))
    }

    /// Whether the reference is the one target and it is a card in the controller's graveyard.
    fn in_your_graveyard(&self, what: &Ref) -> bool {
        *what == Ref::Target(0)
            && self.target_is(|kind| {
                matches!(kind, Targetable::Object(selector)
                    if selector.0.contains(&Pred::In(ZoneKind::Graveyard))
                        && selector.0.contains(&Pred::OwnedBy(Ref::You)))
            })
    }

    /// "put a +1/+1 counter on ~", "put two -1/-1 counters on target creature".
    fn counters(&mut self, on: &Ref, counter: &CounterKind, count: &Amount) -> Option<String> {
        let kind = match counter {
            CounterKind::PlusOnePlusOne => "+1/+1",
            CounterKind::MinusOneMinusOne => "-1/-1",
            _ => return None,
        };
        let Amount::Fixed(number) = count else {
            return None;
        };
        let (many, noun) = match number {
            ..=0 => return None,
            1 => ("a", "counter"),
            _ => (count_word(u32::try_from(*number).ok()?)?, "counters"),
        };
        let place = match on {
            Ref::This => "~".to_owned(),
            Ref::Target(0)
                if self.single(on)
                    && self.target_is(|what| matches!(what, Targetable::Object(_))) =>
            {
                self.name_target()?
            }
            _ => return None,
        };
        Some(format!("put {many} {kind} {noun} on {place}"))
    }

    /// An effect as a sentence without its first capital and its period.
    fn effect(&mut self, effect: &Effect) -> Option<String> {
        Some(match effect {
            Effect::Scry {
                who: Ref::You,
                count,
            } => format!("scry {}", points_text(count)?),
            Effect::Draw {
                who: Ref::You,
                count,
            } => format!("draw {}", cards_text(count)?),
            Effect::Mill {
                who: Ref::You,
                count,
            } => format!("mill {}", cards_text(count)?),
            Effect::Discard {
                who: Ref::You,
                count,
                random,
            } => format!("discard {}{}", cards_text(count)?, at_random(*random)),
            Effect::Draw { who, count } => {
                format!("{} draws {}", self.player(who)?, cards_text(count)?)
            }
            Effect::Mill { who, count } => {
                format!("{} mills {}", self.player(who)?, cards_text(count)?)
            }
            Effect::Discard { who, count, random } => format!(
                "{} discards {}{}",
                self.player(who)?,
                cards_text(count)?,
                at_random(*random)
            ),
            Effect::GainLife { who, amount } => self.life(who, amount, "gain")?,
            Effect::LoseLife { who, amount } => self.life(who, amount, "lose")?,
            Effect::WinGame { who } => format!(
                "{} {} the game",
                self.player(who)?,
                if *who == Ref::You { "win" } else { "wins" }
            ),
            Effect::Move {
                what,
                to: Destination::Hand,
            } if self.in_your_graveyard(what) => {
                format!("return {} to your hand", self.object(what)?)
            }
            Effect::PutCounters { on, counter, count } => self.counters(on, counter, count)?,
            Effect::CreateToken { .. } => create::text(effect)?,
            Effect::DealDamage {
                source: None,
                to,
                amount,
            } => format!(
                "~ deals {} damage to {}",
                points_text(amount)?,
                self.recipient(to)?
            ),
            Effect::Destroy { what, .. } => format!("destroy {}", self.object(what)?),
            Effect::Exile { what } => format!("exile {}", self.object(what)?),
            Effect::Counter { what } => format!("counter {}", self.object(what)?),
            Effect::Continuous {
                affected,
                mods,
                duration: Duration::UntilEndOfTurn,
            } => self.pump(affected, mods)?,
            Effect::May {
                who: Ref::You,
                effects,
            } => match effects.as_slice() {
                [inner] if is_imperative(inner) => format!("you may {}", self.effect(inner)?),
                _ => return None,
            },
            _ => return None,
        })
    }

    /// Who does something: "you", "target player", "each opponent", "its controller".
    fn player(&mut self, who: &Ref) -> Option<String> {
        Some(match who {
            Ref::You => "you".to_owned(),
            Ref::Opponents => "each opponent".to_owned(),
            Ref::EachPlayer => "each player".to_owned(),
            Ref::Target(0)
                if self.target_is(|what| {
                    matches!(what, Targetable::Player | Targetable::Opponent)
                }) =>
            {
                self.name_target()?
            }
            Ref::ControllerOf(inner)
                if **inner == Ref::Target(0)
                    && self.named
                    && self.target_is(|what| matches!(what, Targetable::Object(_))) =>
            {
                "its controller".to_owned()
            }
            _ => return None,
        })
    }

    /// "gain 3 life", "lose X life", "gain life equal to its power".
    fn life(&mut self, who: &Ref, amount: &Amount, verb: &str) -> Option<String> {
        let subject = self.player(who)?;
        let verb = if *who == Ref::You {
            verb.to_owned()
        } else {
            format!("{verb}s")
        };
        let what = match amount {
            Amount::Stat { of, stat } if *of == Ref::Target(0) && self.named => {
                format!("life equal to its {}", stat_name(*stat)?)
            }
            _ => format!("{} life", points_text(amount)?),
        };
        Some(format!("{subject} {verb} {what}"))
    }

    /// The objects a verb acts on: the target in full, or "all creatures".
    fn object(&mut self, what: &Ref) -> Option<String> {
        match what {
            Ref::Target(0) if self.target_is(|what| matches!(what, Targetable::Object(_))) => {
                self.name_target()
            }
            Ref::Each(selector) => Some(format!("all {}", phrase_text(selector, Form::All)?)),
            _ => None,
        }
    }

    /// Who or what is dealt damage.
    fn recipient(&mut self, to: &Ref) -> Option<String> {
        match to {
            Ref::Target(0) => self.name_target(),
            Ref::Opponents => Some("each opponent".to_owned()),
            Ref::EachPlayer => Some("each player".to_owned()),
            Ref::You => Some("you".to_owned()),
            Ref::Each(selector) => Some(format!("each {}", phrase_text(selector, Form::Each)?)),
            _ => None,
        }
    }

    /// "target creature gets +2/+2 and gains flying until end of turn".
    fn pump(&mut self, affected: &Ref, mods: &[Modification]) -> Option<String> {
        let (subject, plural) = match affected {
            Ref::This => ("~".to_owned(), false),
            Ref::Target(0)
                if self.single(affected)
                    && self.target_is(|what| matches!(what, Targetable::Object(_))) =>
            {
                (self.name_target()?, false)
            }
            Ref::Each(selector) => (phrase_text(selector, Form::All)?, true),
            _ => return None,
        };
        let (gets, gains) = if plural {
            ("get", "gain")
        } else {
            ("gets", "gains")
        };
        let (stats, rest) = match mods.split_first() {
            Some((Modification::ModifyPT { power, toughness }, rest)) => (
                Some(format!(
                    "{gets} {}/{}",
                    signed_text(power)?,
                    signed_text(toughness)?
                )),
                rest,
            ),
            _ => (None, mods),
        };
        let keywords = rest
            .iter()
            .map(|modification| match modification {
                Modification::AddKeyword(keyword) => keyword_text(keyword),
                _ => None,
            })
            .collect::<Option<Vec<String>>>()?;
        let mut parts: Vec<String> = stats.into_iter().collect();
        if !keywords.is_empty() {
            parts.push(format!("{gains} {}", keywords.join(" and ")));
        }
        if parts.is_empty() {
            return None;
        }
        Some(format!(
            "{subject} {} until end of turn",
            parts.join(" and ")
        ))
    }
}
