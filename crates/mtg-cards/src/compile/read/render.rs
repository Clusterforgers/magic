//! The renderer: rules data back to the oracle wording the readers read, so a reading can be
//! checked against the text it came from. An effect is a sentence of its own, or the effects of one
//! player in a row are one compound sentence; the readers take ", then" between two steps as the
//! same sequence.

use super::amounts::{cards_text, count_word, points_text, stat_name};
use super::costs;
use super::counters;
use super::create;
use super::effects::{BACK, RETURNED};
use super::library;
use super::modal::{BULLET, DASH};
use super::selectors::{Form, keyword_text, phrase_text, spec_text};
use super::statics;
use super::triggers::{article, condition_text, pattern_text};
use crate::ability::{Ability, Limit, Timing};
use crate::cost::{Cost, CostItem};
use crate::effect::{Destination, Effect, Mode, Order, Place, Produce};
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Condition};

/// The oracle wording of an ability the readers produce, in the first way its effects can be
/// written (see [`variants`]); `None` for any other ability.
pub fn ability(ability: &Ability) -> Option<String> {
    variants(ability).into_iter().next()
}

/// Every wording of an ability the readers produce. The effects of one player in a row are one
/// sentence ("you draw a card and gain 3 life") or several ("draw a card. You gain 3 life"), and
/// the data does not say which a card used, so each way is written. Empty for any other ability.
pub fn variants(ability: &Ability) -> Vec<String> {
    match ability {
        Ability::Spell { targets, effects } => alternatives(targets, effects)
            .iter()
            .map(|sentences| spell(sentences))
            .collect(),
        Ability::Triggered {
            when,
            intervening_if,
            limit: None,
            from: ZoneKind::Battlefield,
            targets,
            effects,
        } => {
            let Some(head) = trigger_head(when, intervening_if.as_ref()) else {
                return Vec::new();
            };
            alternatives(targets, effects)
                .iter()
                .map(|sentences| [head.clone(), closed(sentences, false)].concat().join(" "))
                .collect()
        }
        Ability::Activated {
            cost,
            timing,
            condition: None,
            limit,
            from: ZoneKind::Battlefield,
            targets,
            effects,
        } => {
            let Some(head) = costs::text(cost) else {
                return Vec::new();
            };
            let tails = restriction_texts(*timing, *limit);
            alternatives(targets, effects)
                .iter()
                .flat_map(|sentences| {
                    let body = format!("{head}: {}", closed(sentences, true).join(" "));
                    tails.iter().map(move |tail| {
                        if tail.is_empty() {
                            body.clone()
                        } else {
                            format!("{body} {tail}")
                        }
                    })
                })
                .collect()
        }
        _ => statics::texts(ability),
    }
}

pub(super) fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The ways the effects can be written as sentences, without their first capital and period: the
/// effects of one player in a row may be joined into one sentence, in any run of them.
fn alternatives(targets: &[TargetSpec], effects: &[Effect]) -> Vec<Vec<String>> {
    if targets.len() > 1 || effects.is_empty() {
        return Vec::new();
    }
    if let [
        Effect::Modal {
            choose,
            repeat: false,
            modes,
        },
    ] = effects
    {
        if !targets.is_empty() || !choose.is_one() || modes.len() < 2 {
            return Vec::new();
        }
        return modal_texts(modes)
            .into_iter()
            .map(|text| vec![text])
            .collect();
    }
    // Bit `at` of a mask joins the effect after the one at `at` to it; a long list is tried whole
    // and apart only.
    let joints = effects.len() - 1;
    let masks: Vec<u32> = if joints <= 8 {
        (0..1u32 << joints).collect()
    } else {
        vec![0, u32::MAX >> 1]
    };
    masks
        .into_iter()
        .filter_map(|mask| written(targets, effects, mask))
        .collect()
}

/// The sentences of the effects with the joints of the mask closed into compound sentences.
fn written(targets: &[TargetSpec], effects: &[Effect], mask: u32) -> Option<Vec<String>> {
    let mut writer = Writer {
        targets,
        named: false,
    };
    let mut sentences: Vec<String> = Vec::new();
    let mut start = 0;
    while start < effects.len() {
        // A search and what follows it, or a look at the top cards and the choice from them, are
        // one sentence, which no joint splits.
        if let Some((text, used)) = library::sequence(&effects[start..]) {
            if (mask >> start) & ((1 << used) - 1) != 0 {
                return None;
            }
            sentences.push(text);
            start += used;
            continue;
        }
        // A choice of cards and the move of what was chosen are one sentence, which no joint splits.
        if let [
            choose @ Effect::Choose { .. },
            moved @ Effect::Move { .. },
            ..,
        ] = &effects[start..]
            && let Some(text) =
                put_back_text(choose, moved).or_else(|| returned_text(choose, moved))
        {
            if (mask >> start) & 0b11 != 0 {
                return None;
            }
            sentences.push(text);
            start += 2;
            continue;
        }
        let mut end = start + 1;
        while end < effects.len() && mask & (1 << (end - 1)) != 0 {
            end += 1;
        }
        match &effects[start..end] {
            [effect] => {
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
            group => sentences.push(writer.compound(group)?),
        }
        start = end;
    }
    (targets.is_empty() || writer.named).then_some(sentences)
}

/// "choose one —" and a bullet line for each mode, in every wording of each mode; the last bullet
/// has no period, which the sentence that holds the text adds.
fn modal_texts(modes: &[Mode]) -> Vec<String> {
    let mut texts = vec![format!("choose one {DASH}")];
    for (at, mode) in modes.iter().enumerate() {
        let bullets: Vec<String> = alternatives(&mode.targets, &mode.effects)
            .iter()
            .map(|sentences| {
                let capitalized: Vec<String> = sentences.iter().map(|s| capitalize(s)).collect();
                capitalized.join(". ")
            })
            .collect();
        let stop = if at + 1 == modes.len() { "" } else { "." };
        texts = texts
            .iter()
            .flat_map(|so_far| {
                bullets
                    .iter()
                    .map(move |bullet| format!("{so_far}\n{BULLET} {bullet}{stop}"))
            })
            .take(64)
            .collect();
    }
    texts
}

/// "put two cards from your hand on top of your library in any order": the choice of cards of the
/// hand, bound as `back`, and the move of them.
fn put_back_text(choose: &Effect, moved: &Effect) -> Option<String> {
    let (
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count,
            bind,
            rest_as: None,
        },
        Effect::Move {
            what: Ref::Bound(name),
            to:
                Destination::Library {
                    place: Place::Top,
                    order,
                },
        },
    ) = (choose, moved)
    else {
        return None;
    };
    if bind != BACK || name != BACK || from.0 != [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)]
    {
        return None;
    }
    let Some(Amount::Fixed(most)) = &count.max else {
        return None;
    };
    let number = u32::try_from(*most)
        .ok()
        .filter(|most| *most == count.min)?;
    let tail = match (number, order) {
        (1, None) => String::new(),
        (2.., Some(Order::Chosen)) => " in any order".to_owned(),
        _ => return None,
    };
    let cards = if number == 1 {
        "a card".to_owned()
    } else {
        format!("{} cards", count_word(number)?)
    };
    Some(format!(
        "put {cards} from your hand on top of your library{tail}"
    ))
}

/// A choice of one permanent the player controls, bound as `returned`, and the move of it to its
/// owner's hand: "return a land you control to its owner's hand".
fn returned_text(choose: &Effect, moved: &Effect) -> Option<String> {
    let (
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(from),
            count,
            bind,
            rest_as: None,
        },
        Effect::Move {
            what: Ref::Bound(name),
            to: Destination::Hand,
        },
    ) = (choose, moved)
    else {
        return None;
    };
    if bind != RETURNED || name != RETURNED || !count.is_one() {
        return None;
    }
    let phrase = phrase_text(from, Form::Subject)?;
    Some(format!(
        "return {} {phrase} to its owner's hand",
        article(&phrase)
    ))
}

/// The sentences with their capitals and periods; the first of a triggered ability follows the
/// comma of its event and keeps its lowercase.
fn closed(sentences: &[String], capital_first: bool) -> Vec<String> {
    sentences
        .iter()
        .enumerate()
        .map(|(at, sentence)| {
            if at == 0 && !capital_first {
                format!("{sentence}.")
            } else {
                format!("{}.", capitalize(sentence))
            }
        })
        .collect()
}

/// The sentences of a spell.
fn spell(sentences: &[String]) -> String {
    closed(sentences, true).join(" ")
}

/// The event and an intervening "if" of a triggered ability, each ending in a comma.
fn trigger_head(when: &TriggerPattern, condition: Option<&Condition>) -> Option<Vec<String>> {
    let mut parts = vec![format!("{},", pattern_text(when)?)];
    if let Some(condition) = condition {
        parts.push(format!("if {},", condition_text(condition)?));
    }
    Some(parts)
}

/// The player a verb of the compound sentences is about.
fn player_of(effect: &Effect) -> Option<&Ref> {
    match effect {
        Effect::Draw { who, .. }
        | Effect::Mill { who, .. }
        | Effect::Discard { who, .. }
        | Effect::GainLife { who, .. }
        | Effect::LoseLife { who, .. }
        | Effect::Shuffle { who }
        | Effect::Sacrifice { who, .. } => Some(who),
        _ => None,
    }
}

/// "a", "a and b", "a, b, and c".
fn join_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// "a creature or planeswalker", "an artifact": what a player sacrifices.
fn sacrifice_text(filter: &Selector) -> Option<String> {
    let text = phrase_text(filter, Form::Subject)?;
    Some(format!("{} {text}", article(&text)))
}

/// Whether the effect is written as a verb with "you" understood ("draw a card"), which "you may" can precede.
fn is_imperative(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::Scry { who: Ref::You, .. }
            | Effect::Surveil { who: Ref::You, .. }
            | Effect::Proliferate { who: Ref::You }
            | Effect::Draw { who: Ref::You, .. }
            | Effect::Mill { who: Ref::You, .. }
            | Effect::Discard { who: Ref::You, .. }
            | Effect::Shuffle { who: Ref::You }
            | Effect::Destroy {
                regenerate: true,
                ..
            }
            | Effect::Exile { .. }
            | Effect::Tap { .. }
            | Effect::Untap { .. }
            | Effect::Regenerate { .. }
            | Effect::Counter { .. }
            | Effect::Move { .. }
            | Effect::CreateToken { who: Ref::You, .. }
            | Effect::PutCounters { .. }
            | Effect::RemoveCounters { .. }
            | Effect::MoveCounters { .. }
    )
}

/// "{G}{G}", "one mana of any color": the mana an effect adds.
fn mana_text(mana: &Produce) -> Option<String> {
    Some(match mana {
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
        Produce::ChosenColor(1) => "one mana of the chosen color".to_owned(),
        Produce::OneOf(list) if list.len() > 1 => {
            let symbols: Vec<String> = list
                .iter()
                .map(|mana| format!("{{{}}}", mana.symbol()))
                .collect();
            match symbols.as_slice() {
                [first, second] => format!("{first} or {second}"),
                [init @ .., final_one] => format!("{}, or {final_one}", init.join(", ")),
                [] => return None,
            }
        }
        _ => return None,
    })
}

/// The sentence that restricts when an ability may be activated, in every wording of it; empty
/// text for none.
fn restriction_texts(timing: Timing, limit: Option<Limit>) -> Vec<String> {
    let words = |text: &str| vec![text.to_owned()];
    match (timing, limit) {
        (Timing::Instant, None) => words(""),
        (Timing::Sorcery, None) => words("Activate only as a sorcery."),
        (Timing::Instant, Some(Limit::OncePerTurn)) => words("Activate only once each turn."),
        (Timing::Sorcery, Some(Limit::OncePerTurn)) => vec![
            "Activate only as a sorcery and only once each turn.".to_owned(),
            "Activate only once each turn and only as a sorcery.".to_owned(),
        ],
    }
}

/// "+2", "-3", "+X": a change of power or toughness.
pub(super) fn signed_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Fixed(number) if *number >= 0 => Some(format!("+{number}")),
        Amount::Fixed(number) => Some(number.to_string()),
        Amount::X => Some("+X".to_owned()),
        _ => None,
    }
}

/// Whether the predicate names card types and nothing else.
pub(super) fn is_a_type(pred: &Pred) -> bool {
    match pred {
        Pred::Type(_) => true,
        Pred::AnyOf(types) => types.iter().all(is_a_type),
        _ => false,
    }
}

/// The mana of a payment as the card prints it: `{2}`, `{U}{R}`.
fn cost_text(cost: &Cost) -> Option<String> {
    match cost.0.as_slice() {
        [CostItem::Mana(mana)] if !mana.has_x() => Some(mana.to_string()),
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

    /// Whether the reference is the one target and it is a permanent: not a spell, not a card in a zone.
    fn is_a_permanent(&self, what: &Ref) -> bool {
        self.single(what)
            && self.target_is(|kind| {
                matches!(kind, Targetable::Object(selector)
                    if !selector.0.iter().any(|pred| matches!(pred, Pred::In(_) | Pred::Spell)))
            })
    }

    /// The object counters are put on, taken from or moved: "~", one target, or (with `several`) "each
    /// creature you control".
    fn counter_place(&mut self, place: &Ref, several: bool) -> Option<String> {
        match place {
            Ref::This => Some("~".to_owned()),
            Ref::Target(0)
                if self.single(place)
                    && self.target_is(|what| matches!(what, Targetable::Object(_))) =>
            {
                self.name_target()
            }
            Ref::Each(selector) if several => {
                Some(format!("each {}", phrase_text(selector, Form::Each)?))
            }
            _ => None,
        }
    }

    /// "put a +1/+1 counter on ~", "remove two charge counters from target permanent", "move a
    /// +1/+1 counter from target creature onto ~".
    fn counters(&mut self, effect: &Effect) -> Option<String> {
        Some(match effect {
            Effect::PutCounters { on, counter, count } => {
                let counters = counters::text(counter, count)?;
                format!("put {counters} on {}", self.counter_place(on, true)?)
            }
            Effect::RemoveCounters {
                from,
                counter,
                count,
            } => {
                let counters = counters::text(counter, count)?;
                format!(
                    "remove {counters} from {}",
                    self.counter_place(from, false)?
                )
            }
            Effect::MoveCounters {
                from,
                to,
                counter,
                count,
            } => {
                let counters = counters::text(counter, count)?;
                let from = self.counter_place(from, false)?;
                format!(
                    "move {counters} from {from} onto {}",
                    self.counter_place(to, false)?
                )
            }
            _ => return None,
        })
    }

    /// An effect on a library: "shuffle your library", "look at the top card of your library",
    /// "put target card from your graveyard on the bottom of your library".
    fn library(&mut self, effect: &Effect) -> Option<String> {
        Some(match effect {
            Effect::Shuffle { who: Ref::You } => "shuffle your library".to_owned(),
            Effect::LookAtTop {
                who: Ref::You,
                count,
                bind,
            } => library::look_text(count, bind)?,
            Effect::Move {
                what,
                to: Destination::Library { place, order: None },
            } => {
                let whose = if self.in_your_graveyard(what) {
                    "your"
                } else if self.is_a_permanent(what) {
                    "its owner's"
                } else {
                    return None;
                };
                format!(
                    "put {} {} {whose} library",
                    self.object(what)?,
                    library::place_text(*place)?
                )
            }
            _ => return None,
        })
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
            Effect::Shuffle { who: Ref::You }
            | Effect::LookAtTop { .. }
            | Effect::Move {
                to: Destination::Library { .. },
                ..
            } => self.library(effect)?,
            Effect::Draw { who, .. }
            | Effect::Mill { who, .. }
            | Effect::Discard { who, .. }
            | Effect::GainLife { who, .. }
            | Effect::LoseLife { who, .. }
            | Effect::Shuffle { who }
            | Effect::Sacrifice { who, .. } => {
                let subject = self.player(who)?;
                format!("{subject} {}", self.predicate(effect, *who != Ref::You)?)
            }
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
            Effect::Move {
                what,
                to: Destination::Hand,
            } if self.is_a_permanent(what) => {
                format!("return {} to its owner's hand", self.object(what)?)
            }
            Effect::AddMana {
                who: Ref::You,
                mana,
                times,
            } if times.is_one() => format!("add {}", mana_text(mana)?),
            Effect::PutCounters { .. }
            | Effect::RemoveCounters { .. }
            | Effect::MoveCounters { .. } => self.counters(effect)?,
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
            Effect::Tap { what } => format!("tap {}", self.object(what)?),
            Effect::Untap { what } => format!("untap {}", self.object(what)?),
            Effect::Regenerate { what } => format!("regenerate {}", self.object(what)?),
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
            Effect::Pay {
                who,
                cost,
                paid,
                unpaid,
            } if paid.is_empty() => self.unless_pays(who, cost, unpaid)?,
            _ => return self.keyword_action(effect),
        })
    }

    /// "surveil 2", "proliferate", "you become the monarch": a keyword action or a designation.
    fn keyword_action(&mut self, effect: &Effect) -> Option<String> {
        Some(match effect {
            Effect::Surveil {
                who: Ref::You,
                count,
            } => format!("surveil {}", points_text(count)?),
            Effect::Proliferate { who: Ref::You } => "proliferate".to_owned(),
            Effect::BecomeMonarch { who } => format!(
                "{} {} the monarch",
                self.player(who)?,
                if *who == Ref::You {
                    "become"
                } else {
                    "becomes"
                }
            ),
            _ => return None,
        })
    }

    /// "counter target spell unless its controller pays {2}".
    fn unless_pays(&mut self, who: &Ref, cost: &Cost, unpaid: &[Effect]) -> Option<String> {
        let [inner] = unpaid else {
            return None;
        };
        let happens = self.effect(inner)?;
        let payer = self.player(who)?;
        Some(format!(
            "{happens} unless {payer} pays {}",
            cost_text(cost)?
        ))
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
            Ref::Triggering => "that player".to_owned(),
            _ => return None,
        })
    }

    /// "3 life", "X life", "life equal to its power": what is gained or lost.
    fn life(&self, amount: &Amount) -> Option<String> {
        Some(match amount {
            Amount::Stat { of, stat } if *of == Ref::Target(0) && self.named => {
                format!("life equal to its {}", stat_name(*stat)?)
            }
            _ => format!("{} life", points_text(amount)?),
        })
    }

    /// What a player does, without the subject: "draws a card", "sacrifice a creature"; `third` is
    /// the verb that follows "target opponent" rather than "you".
    fn predicate(&mut self, effect: &Effect, third: bool) -> Option<String> {
        let s = if third { "s" } else { "" };
        Some(match effect {
            Effect::Draw { count, .. } => format!("draw{s} {}", cards_text(count)?),
            Effect::Mill { count, .. } => format!("mill{s} {}", cards_text(count)?),
            Effect::Discard { count, random, .. } => {
                format!("discard{s} {}{}", cards_text(count)?, at_random(*random))
            }
            Effect::GainLife { amount, .. } => format!("gain{s} {}", self.life(amount)?),
            Effect::LoseLife { amount, .. } => format!("lose{s} {}", self.life(amount)?),
            Effect::Sacrifice { filter, count, .. } if count.is_one() => {
                format!("sacrifice{s} {}", sacrifice_text(filter)?)
            }
            Effect::Shuffle { who } => {
                format!(
                    "shuffle{s} {} library",
                    if *who == Ref::You { "your" } else { "their" }
                )
            }
            _ => return None,
        })
    }

    /// Several effects of one player as one sentence: "target opponent discards a card and loses 3 life".
    fn compound(&mut self, group: &[Effect]) -> Option<String> {
        let who = player_of(group.first()?)?;
        if group.iter().any(|effect| player_of(effect) != Some(who)) {
            return None;
        }
        let subject = self.player(who)?;
        let mut phrases = Vec::with_capacity(group.len());
        for effect in group {
            phrases.push(self.predicate(effect, *who != Ref::You)?);
        }
        Some(format!("{subject} {}", join_list(&phrases)))
    }

    /// The objects a verb acts on: the target in full, or "all creatures".
    fn object(&mut self, what: &Ref) -> Option<String> {
        match what {
            Ref::Target(0) if self.target_is(|what| matches!(what, Targetable::Object(_))) => {
                self.name_target()
            }
            Ref::Each(selector) => Some(format!("all {}", phrase_text(selector, Form::All)?)),
            Ref::This => Some("~".to_owned()),
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
            Ref::Each(selector) => {
                let phrase = phrase_text(selector, Form::All)?;
                // "All creatures get", but "Creatures you control get" and "Attacking creatures get".
                let all = selector.0.iter().all(is_a_type);
                (if all { format!("all {phrase}") } else { phrase }, true)
            }
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
