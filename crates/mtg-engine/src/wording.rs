//! Words for what the rules data says, for the prompts of players: a target specification ("up to two target
//! creatures you control"), the event a triggered ability waits for ("When it enters the battlefield"), what a
//! discard or a sacrifice asks ("Discard a card."), a "may" ("Draw a card?") and a choice ("Choose an opponent.").
//!
//! They are hints for a person choosing, not rules: a predicate with no short phrase is left out.

use mtg_cards::effect::Effect;
use mtg_cards::refs::{AbilityKind, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::{Amount, CastWay, Cmp, Condition, DayNight, StatKind};
use mtg_core::ids::PlayerId;

/// "a" or "an" ahead of `word`.
fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    }
}

fn lower(debug: &impl std::fmt::Debug) -> String {
    format!("{debug:?}").to_lowercase()
}

fn stat_word(stat: StatKind) -> &'static str {
    match stat {
        StatKind::Power => "power",
        StatKind::Toughness => "toughness",
        StatKind::ManaValue => "mana value",
        StatKind::Loyalty => "loyalty",
        StatKind::Colors => "colors",
    }
}

fn comparison(stat: StatKind, op: Cmp, to: i32) -> String {
    let stat = stat_word(stat);
    match op {
        Cmp::Eq => format!("with {stat} {to}"),
        Cmp::Ne => format!("with {stat} other than {to}"),
        Cmp::Lt => format!("with {stat} less than {to}"),
        Cmp::Le => format!("with {stat} {to} or less"),
        Cmp::Gt => format!("with {stat} greater than {to}"),
        Cmp::Ge => format!("with {stat} {to} or greater"),
    }
}

fn zone_phrase(kind: ZoneKind, yours: bool) -> String {
    let (place, owner) = match kind {
        ZoneKind::Battlefield => return "on the battlefield".to_owned(),
        ZoneKind::Stack => return "on the stack".to_owned(),
        ZoneKind::Exile => return "in exile".to_owned(),
        ZoneKind::Hand => ("hand", "a"),
        ZoneKind::Library => ("library", "a"),
        ZoneKind::Graveyard => ("graveyard", "a"),
        ZoneKind::Command => ("command zone", "a"),
    };
    format!("in {} {place}", if yours { "your" } else { owner })
}

/// A noun phrase for what a selector matches, split into the part that is pluralised and the part after it:
/// ("tapped creature", "you control").
fn phrase(selector: &Selector) -> (String, String) {
    let (mut adjectives, mut nouns, mut tail) = (Vec::new(), Vec::new(), Vec::new());
    let (mut zone, mut yours, mut this) = (None, false, false);
    for pred in &selector.0 {
        match pred {
            Pred::This => this = true,
            Pred::Other => adjectives.push("other".to_owned()),
            Pred::Type(card_type) => nouns.push(lower(card_type)),
            Pred::Subtype(subtype) => nouns.push(subtype.0.clone()),
            Pred::Supertype(supertype) => adjectives.push(lower(supertype)),
            Pred::Color(color) => adjectives.push(lower(color)),
            Pred::Colorless => adjectives.push("colorless".to_owned()),
            Pred::Token => adjectives.push("token".to_owned()),
            Pred::Commander => nouns.push("commander".to_owned()),
            Pred::Spell => nouns.push("spell".to_owned()),
            Pred::Ability(AbilityKind::Activated) => nouns.push("activated ability".to_owned()),
            Pred::Ability(AbilityKind::Triggered) => nouns.push("triggered ability".to_owned()),
            Pred::Tapped => adjectives.push("tapped".to_owned()),
            Pred::Untapped => adjectives.push("untapped".to_owned()),
            Pred::Monstrous => adjectives.push("monstrous".to_owned()),
            Pred::EnteredThisTurn => tail.push("that entered this turn".to_owned()),
            Pred::Attacking => adjectives.push("attacking".to_owned()),
            Pred::AttackingPlayer(Ref::AttachedPlayer) => {
                tail.push("attacking enchanted player".to_owned());
            }
            Pred::AttackingPlayer(Ref::You) => tail.push("attacking you".to_owned()),
            Pred::Blocking => adjectives.push("blocking".to_owned()),
            Pred::Unblocked => adjectives.push("unblocked".to_owned()),
            Pred::ControlledBy(Ref::You) => tail.push("you control".to_owned()),
            Pred::ControlledBy(Ref::Opponents) => tail.push("an opponent controls".to_owned()),
            Pred::OwnedBy(Ref::You) => yours = true,
            Pred::OwnedBy(Ref::Opponents) => tail.push("an opponent owns".to_owned()),
            Pred::In(kind) => zone = Some(*kind),
            Pred::Named(name) => nouns.push(format!("card named {name}")),
            Pred::HasKeyword(keyword) => {
                tail.push(format!("with {}", keyword.name().to_lowercase()));
            }
            Pred::HasCounter(kind) => tail.push(format!("with a {} counter on it", kind.name())),
            Pred::Compare {
                stat,
                op,
                to: Amount::Fixed(to),
            } => tail.push(comparison(*stat, *op, *to)),
            Pred::AnyOf(any)
                if any
                    .iter()
                    .all(|p| matches!(p, Pred::Type(_) | Pred::Subtype(_))) =>
            {
                let words: Vec<String> = any
                    .iter()
                    .filter_map(|p| match p {
                        Pred::Type(card_type) => Some(lower(card_type)),
                        Pred::Subtype(subtype) => Some(subtype.0.clone()),
                        _ => None,
                    })
                    .collect();
                nouns.push(words.join(" or "));
            }
            _ => {}
        }
    }
    match zone {
        Some(kind) => tail.push(zone_phrase(kind, yours)),
        None if yours => tail.push("you own".to_owned()),
        None => {}
    }
    let noun = if this {
        "permanent".to_owned()
    } else if nouns.is_empty() {
        if zone.is_some() { "card" } else { "permanent" }.to_owned()
    } else {
        nouns.join(" ")
    };
    adjectives.push(noun);
    (adjectives.join(" "), tail.join(" "))
}

fn join(head: &str, tail: &str) -> String {
    if tail.is_empty() {
        head.to_owned()
    } else {
        format!("{head} {tail}")
    }
}

/// What a selector matches as a noun phrase: "creature you control".
fn noun(selector: &Selector) -> String {
    let (head, tail) = phrase(selector);
    join(&head, &tail)
}

/// A selector as the subject of a clause: "it" for the source itself, else "a creature you control".
fn subject(selector: &Selector) -> String {
    if selector.0 == [Pred::This] {
        return "it".to_owned();
    }
    let noun = noun(selector);
    format!("{} {noun}", article(&noun))
}

/// Who, as the subject of a verb that takes an "s" after a third person: ("you", "") or ("an opponent", "s").
fn who(by: &Ref) -> (&'static str, &'static str) {
    match by {
        Ref::You => ("you", ""),
        Ref::Opponents => ("an opponent", "s"),
        _ => ("a player", "s"),
    }
}

fn kind_of(what: &Targetable) -> (String, String) {
    match what {
        Targetable::AnyTarget => ("target".to_owned(), String::new()),
        Targetable::Player => ("player".to_owned(), String::new()),
        Targetable::Opponent => ("opponent".to_owned(), String::new()),
        Targetable::Object(selector) => phrase(selector),
        Targetable::Either(kinds) => (
            kinds
                .iter()
                .map(|kind| join(&kind_of(kind).0, &kind_of(kind).1))
                .collect::<Vec<_>>()
                .join(" or "),
            String::new(),
        ),
    }
}

/// A target specification in the words of a card: "target creature you control", "up to two target players",
/// "any target".
pub(crate) fn spec(spec: &TargetSpec) -> String {
    let (head, tail) = kind_of(&spec.what);
    let any = matches!(spec.what, Targetable::AnyTarget);
    let one = |head: &str| {
        if any {
            "any target".to_owned()
        } else {
            join(&format!("target {head}"), &tail)
        }
    };
    let several = |head: &str| {
        if any {
            "targets".to_owned()
        } else if head.contains(" or ") {
            join(&format!("target {head}"), &tail)
        } else {
            join(&format!("target {head}s"), &tail)
        }
    };
    let (min, max) = (spec.bounds.min, &spec.bounds.max);
    match max {
        Some(Amount::Fixed(1)) if min == 1 => one(&head),
        Some(Amount::Fixed(1)) => {
            format!("up to one {}", one(&head).replace("any target", "target"))
        }
        Some(Amount::Fixed(n)) if i64::from(min) == i64::from(*n) => {
            format!("{n} {}", several(&head))
        }
        Some(Amount::Fixed(n)) if min == 0 => format!("up to {n} {}", several(&head)),
        Some(Amount::Fixed(n)) => format!("{min} to {n} {}", several(&head)),
        _ if min == 0 => format!("any number of {}", several(&head)),
        _ => format!("{min} or more {}", several(&head)),
    }
}

fn step_words(step: Step) -> &'static str {
    match step {
        Step::Untap => "untap step",
        Step::Upkeep => "upkeep",
        Step::Draw => "draw step",
        Step::Main1 => "precombat main phase",
        Step::BeginCombat => "combat",
        Step::DeclareAttackers => "declare attackers step",
        Step::DeclareBlockers => "declare blockers step",
        Step::FirstStrikeDamage => "first-strike damage step",
        Step::CombatDamage => "combat damage step",
        Step::EndCombat => "end of combat step",
        Step::Main2 => "postcombat main phase",
        Step::End => "end step",
        Step::Cleanup => "cleanup step",
    }
}

/// The state a state trigger waits for, as a card says it after "When": "you control no Islands", "you have no cards
/// in hand", "you have 10 or less life"; a state with no short phrase is "its state is true".
fn state_words(condition: &Condition) -> String {
    match condition {
        Condition::Compare {
            left: Amount::Count(selector),
            op: Cmp::Eq,
            right: Amount::Fixed(0),
        } => {
            if selector.0.contains(&Pred::In(ZoneKind::Hand))
                && selector.0.contains(&Pred::OwnedBy(Ref::You))
            {
                return "you have no cards in hand".to_owned();
            }
            let (head, tail) = phrase(selector);
            if tail == "you control" {
                format!("you control no {}", plural(&head))
            } else {
                format!("there are no {}", join(&plural(&head), &tail))
            }
        }
        Condition::Compare {
            left: Amount::Life(Ref::You),
            op: op @ (Cmp::Le | Cmp::Ge),
            right: Amount::Fixed(life),
        } => format!(
            "you have {life} or {} life",
            if *op == Cmp::Le { "less" } else { "more" }
        ),
        _ => "its state is true".to_owned(),
    }
}

/// The clause of one pattern, such as "it enters the battlefield", and whether it speaks of the source itself.
#[allow(clippy::too_many_lines)]
fn clause(pattern: &TriggerPattern) -> (String, bool) {
    match pattern {
        TriggerPattern::State { condition } => (state_words(condition), true),
        TriggerPattern::ZoneChange { what, from, to } => {
            let moves = match (from, to) {
                (_, Some(ZoneKind::Battlefield)) => "enters the battlefield".to_owned(),
                (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard)) => "dies".to_owned(),
                (Some(ZoneKind::Battlefield), None) => "leaves the battlefield".to_owned(),
                (None, Some(ZoneKind::Graveyard)) => "is put into a graveyard".to_owned(),
                (Some(ZoneKind::Library), Some(ZoneKind::Hand)) => "is drawn".to_owned(),
                (Some(from), Some(to)) => format!(
                    "moves from {} to {}",
                    zone_phrase(*from, false).trim_start_matches("in "),
                    zone_phrase(*to, false).trim_start_matches("in ")
                ),
                _ => "changes zones".to_owned(),
            };
            (format!("{} {moves}", subject(what)), what.0 == [Pred::This])
        }
        TriggerPattern::Attacks { what } => {
            (format!("{} attacks", subject(what)), what.0 == [Pred::This])
        }
        TriggerPattern::AttacksAlone { what } => (
            format!("{} attacks alone", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::AttacksPlayer { what, at } => {
            let whom = match at {
                Ref::MostLife => "the player with the most life",
                Ref::AttachedPlayer => "enchanted player",
                Ref::You => "you",
                Ref::Monarch => "the monarch",
                _ => "a player",
            };
            (format!("{} attacks {whom}", subject(what)), false)
        }
        TriggerPattern::PlayerAttacked { who: attacked } => {
            let text = match attacked {
                Ref::You => "you are attacked".to_owned(),
                Ref::AttachedPlayer => "enchanted player is attacked".to_owned(),
                other => format!("{} is attacked", who(other).0),
            };
            (text, false)
        }
        TriggerPattern::PlayerAttacks { by, at } => {
            let (name, s) = match by {
                Ref::AttachedPlayer => ("enchanted player", "s"),
                other => who(other),
            };
            let whom = match at {
                Ref::AttachedPlayer => "enchanted player",
                Ref::You => "you",
                _ => "a player",
            };
            (
                format!("{name} attack{s} {whom} with one or more creatures"),
                false,
            )
        }
        TriggerPattern::BecomesTarget { what, by } => {
            let whose = match by.0.as_slice() {
                [Pred::ControlledBy(Ref::Opponents)] => " an opponent controls",
                [Pred::ControlledBy(Ref::You)] => " you control",
                _ => "",
            };
            (
                format!(
                    "{} becomes the target of a spell or ability{whose}",
                    subject(what)
                ),
                what.0 == [Pred::This],
            )
        }
        TriggerPattern::SpellCast { what, by } => {
            let (who, s) = who(by);
            let spell = if what.is_empty() {
                "a spell".to_owned()
            } else {
                let noun = noun(what);
                format!("{} {noun} spell", article(&noun))
            };
            (format!("{who} cast{s} {spell}"), false)
        }
        TriggerPattern::Sacrificed { what, by } => {
            let (who, s) = who(by);
            (format!("{who} sacrifice{s} {}", subject(what)), false)
        }
        TriggerPattern::StepBegins { step, whose } => {
            let owner = match whose {
                Ref::You => "your",
                Ref::Opponents => "an opponent's",
                Ref::Monarch => "the monarch's",
                _ => "each",
            };
            let when = if *step == Step::BeginCombat {
                format!("the beginning of combat on {owner} turn").replace("each turn", "each turn")
            } else {
                format!("the beginning of {owner} {}", step_words(*step))
            };
            (when, false)
        }
        TriggerPattern::DealsDamage {
            source,
            to: Some(hit @ (Ref::This | Ref::Each(_))),
            combat: None,
        } if source.is_empty() => {
            let recipient = match hit {
                Ref::Each(what) => subject(what),
                _ => "it".to_owned(),
            };
            (format!("{recipient} is dealt damage"), *hit == Ref::This)
        }
        TriggerPattern::DealsDamage { source, combat, .. } => {
            let kind = match combat {
                Some(true) => "combat damage",
                Some(false) => "noncombat damage",
                None => "damage",
            };
            (
                format!("{} deals {kind}", subject(source)),
                source.0 == [Pred::This],
            )
        }
        TriggerPattern::LifeGained { who: by } => {
            let (name, s) = who(by);
            (format!("{name} gain{s} life"), false)
        }
        TriggerPattern::LifeLost { who: by } => {
            let (name, s) = who(by);
            (format!("{name} lose{s} life"), false)
        }
        TriggerPattern::BecomesMonarch { who: by } => {
            let (name, s) = who(by);
            (format!("{name} become{s} the monarch"), false)
        }
        TriggerPattern::DayNightChanges { to } => (
            match to {
                None => "day becomes night or night becomes day".to_owned(),
                Some(DayNight::Day) => "night becomes day".to_owned(),
                Some(DayNight::Night) => "day becomes night".to_owned(),
            },
            false,
        ),
        TriggerPattern::Proliferated { who: by } => {
            let (name, s) = who(by);
            (format!("{name} proliferate{s}"), false)
        }
        TriggerPattern::Amassed { who: by } => {
            let (name, s) = who(by);
            (
                format!("{name} amass{}", if s.is_empty() { "" } else { "es" }),
                false,
            )
        }
        TriggerPattern::Bolstered { who: by } => {
            let (name, s) = who(by);
            (
                format!("{name} bolster{}", if s.is_empty() { "" } else { "s" }),
                false,
            )
        }
        TriggerPattern::Surveilled { who: by } => {
            let (name, s) = who(by);
            (
                format!("{name} surveil{}", if s.is_empty() { "" } else { "s" }),
                false,
            )
        }
        TriggerPattern::ZoneChangeExcept { what, except, to } => {
            let moves = match to {
                Some(to) => format!(
                    "moves to {} from anywhere but {}",
                    zone_phrase(*to, false).trim_start_matches("in "),
                    zone_phrase(*except, false)
                        .trim_start_matches("in ")
                        .trim_start_matches("on ")
                ),
                None => format!(
                    "leaves a zone other than {}",
                    zone_phrase(*except, false)
                        .trim_start_matches("in ")
                        .trim_start_matches("on ")
                ),
            };
            (format!("{} {moves}", subject(what)), false)
        }
        TriggerPattern::Drew { who: by, what } => {
            let (name, s) = who(by);
            let card = if what.is_empty() {
                "a card".to_owned()
            } else {
                subject(what)
            };
            (format!("{name} draw{s} {card}"), false)
        }
        TriggerPattern::Discarded { what, by } => {
            let (name, s) = who(by);
            let card = if what.is_empty() {
                "a card".to_owned()
            } else {
                subject(what)
            };
            (format!("{name} discard{s} {card}"), false)
        }
        TriggerPattern::Tapped { what } => (
            format!("{} becomes tapped", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::Untapped { what } => (
            format!("{} becomes untapped", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::Explored { what } => (
            format!("{} explores", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::BecomesMonstrous { what } => (
            format!("{} becomes monstrous", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::BecomesCrewed { what } => (
            format!("{} becomes crewed", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::BecomesSaddled { what } => (
            format!("{} becomes saddled", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::BecomesLevel { what, level } => (
            format!("{} becomes level {level}", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::TurnedFaceUp { what } => (
            format!("{} is turned face up", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::Mutates { what } => {
            (format!("{} mutates", subject(what)), what.0 == [Pred::This])
        }
        TriggerPattern::Adapted { what } => {
            (format!("{} adapts", subject(what)), what.0 == [Pred::This])
        }
        TriggerPattern::Connived { what } => (
            format!("{} connives", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::Cycled { what, by } => {
            let (name, s) = who(by);
            let card = if what.is_empty() {
                "a card".to_owned()
            } else {
                subject(what)
            };
            (format!("{name} cycle{s} {card}"), what.0 == [Pred::This])
        }
        TriggerPattern::CountersRemoved { what, .. } => {
            let from = if what.is_empty() {
                "a permanent".to_owned()
            } else {
                subject(what)
            };
            (format!("counters are removed from {from}"), false)
        }
        TriggerPattern::Chapter { number } => {
            (format!("its lore counters reach chapter {number}"), true)
        }
        TriggerPattern::Attached { what, to } => {
            let attachment = if what.is_empty() {
                "an attachment".to_owned()
            } else {
                subject(what)
            };
            let host = if to.is_empty() {
                "a permanent".to_owned()
            } else {
                subject(to)
            };
            (
                format!("{attachment} becomes attached to {host}"),
                what.0 == [Pred::This],
            )
        }
        TriggerPattern::TappedForMana { what, by } => {
            let (name, s) = who(by);
            (format!("{name} tap{s} {} for mana", subject(what)), false)
        }
        TriggerPattern::Blocks { what } => {
            (format!("{} blocks", subject(what)), what.0 == [Pred::This])
        }
        TriggerPattern::BecomesBlocked { what } => (
            format!("{} becomes blocked", subject(what)),
            what.0 == [Pred::This],
        ),
        TriggerPattern::GroupAttacks { what, at_least } => {
            let (head, tail) = phrase(what);
            (
                format!(
                    "{} attack",
                    join(&format!("{at_least} or more {}", plural(&head)), &tail)
                ),
                false,
            )
        }
        TriggerPattern::CountersPut { what, .. } => {
            let on = if what.is_empty() {
                "a permanent".to_owned()
            } else {
                subject(what)
            };
            (format!("counters are put on {on}"), false)
        }
        TriggerPattern::Batch(inner) => (
            format!("one or more events happen together: {}", clause(inner).0),
            false,
        ),
        TriggerPattern::Nth { event, nth } => {
            (format!("{} (time {nth} this turn)", clause(event).0), false)
        }
        TriggerPattern::AnyOf(patterns) => {
            let clauses: Vec<(String, bool)> = patterns.iter().map(clause).collect();
            (
                clauses
                    .iter()
                    .map(|(text, _)| text.as_str())
                    .collect::<Vec<_>>()
                    .join(" or "),
                clauses.iter().all(|(_, this)| *this),
            )
        }
    }
}

/// The event a triggered ability waits for, as its card would say it: "When it enters the battlefield", "Whenever
/// an opponent casts a spell", "At the beginning of your upkeep".
pub(crate) fn pattern(pattern: &TriggerPattern) -> String {
    let (text, itself) = clause(pattern);
    if text.starts_with("the beginning of") {
        return format!("At {text}");
    }
    format!("{} {text}", if itself { "When" } else { "Whenever" })
}

/// The words of a trigger that a keyword adds: the keyword's name, then the event as a card would say it in the
/// middle of a sentence ("Ward: whenever it becomes the target of a spell or ability").
pub(crate) fn keyword_trigger(keyword: &str, said: &str) -> String {
    let mut letters = said.chars();
    let rest: String = letters
        .next()
        .map(|first| first.to_lowercase().chain(letters).collect())
        .unwrap_or_default();
    format!("{keyword}: {rest}")
}

/// The plural of a noun phrase made of one or more alternatives: "creatures or planeswalkers".
fn plural(head: &str) -> String {
    head.split(" or ")
        .map(|word| format!("{word}s"))
        .collect::<Vec<_>>()
        .join(" or ")
}

/// What a discard asks of the player who chooses, as a card says it: "Discard a card.", "Discard 2 cards."
pub(crate) fn discard(count: usize) -> String {
    if count == 1 {
        "Discard a card.".to_owned()
    } else {
        format!("Discard {count} cards.")
    }
}

/// What a sacrifice asks of the player who chooses: "Sacrifice a creature or planeswalker.", "Sacrifice 2 creatures."
pub(crate) fn sacrifice(filter: &Selector, count: usize) -> String {
    if count == 1 {
        return format!("Sacrifice {}.", subject(filter));
    }
    let (head, tail) = phrase(filter);
    format!("Sacrifice {count} {}.", join(&plural(&head), &tail))
}

/// `count` of what `filter` describes, with `place` after them: "a creature you control", "2 creatures you control".
fn counted(filter: &Selector, count: usize, extra: &str) -> String {
    let (head, tail) = phrase(filter);
    let tail = if tail.is_empty() {
        extra.to_owned()
    } else {
        format!("{tail} {extra}")
    };
    if count == 1 {
        format!("{} {}", article(&head), join(&head, &tail))
    } else {
        format!("{count} {}", join(&plural(&head), &tail))
    }
}

/// What a cost that exiles cards from the payer's graveyard asks: "Exile a creature card from your graveyard."
pub(crate) fn exile_from_graveyard(filter: &Selector, count: usize) -> String {
    take_cards("Exile", filter, count, "graveyard")
}

/// What a cost that exiles cards from the payer's hand asks: "Exile a red card from your hand."
pub(crate) fn exile_from_hand(filter: &Selector, count: usize) -> String {
    take_cards("Exile", filter, count, "hand")
}

/// What a cost that reveals cards from the payer's hand asks: "Reveal a Giant card from your hand."
pub(crate) fn reveal_from_hand(filter: &Selector, count: usize) -> String {
    take_cards("Reveal", filter, count, "hand")
}

/// "Exile a creature card from your graveyard.": `count` cards that match `filter` from `place`, a zone of the payer.
fn take_cards(verb: &str, filter: &Selector, count: usize, place: &str) -> String {
    let (head, _) = phrase(filter);
    let kind = if head == "permanent" {
        "card".to_owned()
    } else {
        format!("{head} card")
    };
    let what = if count == 1 {
        format!("{} {kind}", article(&kind))
    } else {
        format!("{count} {kind}s")
    };
    format!("{verb} {what} from your {place}.")
}

/// What a cost that taps permanents the payer controls asks: "Tap an untapped creature you control."
pub(crate) fn tap_untapped(filter: &Selector, count: usize) -> String {
    let (head, tail) = phrase(filter);
    let untapped = format!("untapped {head}");
    let what = if count == 1 {
        format!("{} {}", article(&untapped), join(&untapped, &tail))
    } else {
        format!("{count} {}", join(&plural(&untapped), &tail))
    };
    format!("Tap {what} you control.")
}

/// What a cost that taps permanents for their power asks: "Tap any number of untapped creatures you control with total
/// power 3 or more."
pub(crate) fn tap_power(filter: &Selector, power: usize) -> String {
    let (head, tail) = phrase(filter);
    let untapped = join(&plural(&format!("untapped {head}")), &tail);
    format!("Tap any number of {untapped} you control with total power {power} or more.")
}

/// What a cost that returns permanents the payer controls to hand asks: "Return a land you control to its owner's hand."
pub(crate) fn return_to_hand(filter: &Selector, count: usize) -> String {
    let what = counted(filter, count, "you control");
    let owner = if count == 1 { "its" } else { "their" };
    format!("Return {what} to {owner} owner's hand.")
}

/// What a search asks of the player who chooses, as a card says it: "Search your library for a basic land card.",
/// "Search your library for up to 2 basic land cards." `most` is `usize::MAX` for any number.
pub(crate) fn search(zone: ZoneKind, filter: &Selector, min: usize, most: usize) -> String {
    let place = match zone {
        ZoneKind::Library => "your library",
        ZoneKind::Hand => "your hand",
        ZoneKind::Graveyard => "your graveyard",
        ZoneKind::Exile => "exile",
        ZoneKind::Battlefield => "the battlefield",
        ZoneKind::Stack => "the stack",
        ZoneKind::Command => "your command zone",
    };
    let described = Selector(
        filter
            .0
            .iter()
            .filter(|pred| !matches!(pred, Pred::In(_)))
            .cloned()
            .collect(),
    );
    let (head, tail) = phrase(&described);
    let kind = if head == "permanent" {
        "card".to_owned()
    } else if head.contains("card named") {
        head
    } else {
        format!("{head} card")
    };
    let what = if most == 1 {
        format!("{} {}", article(&kind), join(&kind, &tail))
    } else {
        let several = match kind.strip_prefix("card named ") {
            Some(named) => format!("cards named {named}"),
            None => format!("{kind}s"),
        };
        let several = join(&several, &tail);
        if most == usize::MAX {
            format!("any number of {several}")
        } else if min == most {
            format!("{most} {several}")
        } else {
            format!("up to {most} {several}")
        }
    };
    format!("Search {place} for {what}.")
}

/// What an Aura asks as it enters the battlefield without being cast (CR 303.4g).
pub(crate) const ENCHANT: &str = "Choose what the Aura enchants.";

/// What a scry asks about the cards looked at.
pub(crate) const SCRY: &str = "Put any number of the cards on the bottom of your library.";

/// What a surveil asks about the cards looked at.
pub(crate) const SURVEIL: &str = "Put any number of the cards into your graveyard.";

/// What a proliferate asks about the permanents with counters.
pub(crate) const PROLIFERATE: &str =
    "Choose any number of permanents with counters to proliferate.";

/// What a proliferate asks about a player who holds counters: `holder` is one of them, `proliferating` the one who chooses.
pub(crate) fn proliferate_player(proliferating: PlayerId, holder: PlayerId) -> String {
    if proliferating == holder {
        "Proliferate yourself: another counter of each kind you have?".to_owned()
    } else {
        format!(
            "Proliferate seat {}: another counter of each kind they have?",
            holder.index() + 1
        )
    }
}

/// What an explore asks about the revealed card that is not a land.
pub(crate) const EXPLORE: &str = "You may put the revealed card into your graveyard.";

/// What the order of cards put into a library asks: "Put the cards on top of your library in any order."
pub(crate) fn library_order(bottom: bool) -> String {
    let place = if bottom { "the bottom" } else { "top" };
    format!("Put the cards on {place} of your library in any order.")
}

/// "a card" or "3 cards".
fn cards(count: i32) -> String {
    if count == 1 {
        "a card".to_owned()
    } else {
        format!("{count} cards")
    }
}

/// What one effect that the player who decides does to themselves reads like as a verb phrase: "draw a card", "gain 3
/// life". An effect with no short phrase yet, or one done to or by someone else, has none.
fn verb_phrase(effect: &Effect) -> Option<String> {
    Some(match effect {
        Effect::Draw {
            who: Ref::You,
            count: Amount::Fixed(n),
        } => format!("draw {}", cards(*n)),
        Effect::Discard {
            who: Ref::You,
            count: Amount::Fixed(n),
            random: false,
        } => format!("discard {}", cards(*n)),
        Effect::Mill {
            who: Ref::You,
            count: Amount::Fixed(n),
        } => format!("mill {}", cards(*n)),
        Effect::Scry {
            who: Ref::You,
            count: Amount::Fixed(n),
        } => format!("scry {n}"),
        Effect::Surveil {
            who: Ref::You,
            count: Amount::Fixed(n),
        } => format!("surveil {n}"),
        Effect::GainLife {
            who: Ref::You,
            amount: Amount::Fixed(n),
        } => format!("gain {n} life"),
        Effect::LoseLife {
            who: Ref::You,
            amount: Amount::Fixed(n),
        } => format!("lose {n} life"),
        _ => return None,
    })
}

/// The question of a "may": what the effects do as a card says it ("Draw a card?", "Draw a card and gain 3 life?").
/// It is empty when an effect has no phrase yet, and the prompt then asks in general words.
pub(crate) fn may(effects: &[Effect]) -> String {
    let phrases: Option<Vec<String>> = effects.iter().map(verb_phrase).collect();
    let Some(mut phrases) = phrases.filter(|phrases| !phrases.is_empty()) else {
        return String::new();
    };
    let last = phrases.pop().unwrap_or_default();
    let mut text = if phrases.is_empty() {
        last
    } else {
        format!("{} and {last}", phrases.join(", "))
    };
    if let Some(first) = text.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    format!("{text}?")
}

/// The question of a payment an effect offers, as a card says it, from the parts of its cost: "Pay {2}?", "Pay {1}, discard
/// a card?"
pub(crate) fn pay(parts: &[String]) -> String {
    let mut question = String::new();
    for (at, part) in parts.iter().enumerate() {
        if at > 0 {
            question.push_str(", ");
        }
        let mut letters = part.chars();
        match (at, letters.next()) {
            (0, Some(first)) => question.push(first),
            (_, Some(first)) => question.extend(first.to_lowercase()),
            _ => {}
        }
        question.push_str(letters.as_str());
    }
    question.push('?');
    question
}

/// The question of a cast an effect allows: "Cast Lightning Bolt without paying its mana cost?"
/// `alternative` is the way and the mana of a cost the card names (madness, miracle).
pub(crate) fn cast(name: &str, free: bool, alternative: Option<(CastWay, &str)>) -> String {
    if let Some((way, cost)) = alternative {
        let way = match way {
            CastWay::Miracle => "miracle",
            _ => "madness",
        };
        format!("Cast {name} for its {way} cost {cost}?")
    } else if free {
        format!("Cast {name} without paying its mana cost?")
    } else {
        format!("Cast {name}?")
    }
}

/// The question of a play an effect allows: "Play Forest without paying its mana cost?"
pub(crate) fn play(name: &str, free: bool) -> String {
    if free {
        format!("Play {name} without paying its mana cost?")
    } else {
        format!("Play {name}?")
    }
}

/// What a choice among players asks: "Choose an opponent." or "Choose a player."
pub(crate) fn choose_player(from: &Ref) -> String {
    match from {
        Ref::Opponents => "Choose an opponent.".to_owned(),
        _ => "Choose a player.".to_owned(),
    }
}

/// What a choice of objects asks, from the fewest to the most: "Choose a creature in your hand.", "Choose 2 cards in
/// your hand.", "Choose up to 2 cards."
pub(crate) fn choose_objects(from: &Ref, min: usize, most: usize) -> String {
    let (head, tail) = match from {
        Ref::Each(selector) => phrase(selector),
        _ => ("card".to_owned(), String::new()),
    };
    if most == 1 && min == 1 {
        let noun = join(&head, &tail);
        return format!("Choose {} {noun}.", article(&noun));
    }
    let up_to = if min == most { "" } else { "up to " };
    format!("Choose {up_to}{most} {}.", join(&plural(&head), &tail))
}

#[cfg(test)]
mod tests {
    //! Goal: target specifications, trigger patterns and the questions of discards, sacrifices, choices and library orders read as the cards say them, for the shapes the corpus and the prompts use, and a predicate with no phrase is left out instead of breaking the sentence.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::on_top(false, "Put the cards on top of your library in any order.")]
    #[case::on_the_bottom(true, "Put the cards on the bottom of your library in any order.")]
    fn the_order_of_cards_put_into_a_library_says_where(
        #[case] bottom: bool,
        #[case] expected: &str,
    ) {
        assert_eq!(library_order(bottom), expected);
    }

    #[rstest]
    #[case::generic(&["Pay {2}"], "Pay {2}?")]
    #[case::coloured(&["Pay {1}{R}"], "Pay {1}{R}?")]
    #[case::hybrid(&["Pay {G/W}{G/W}"], "Pay {G/W}{G/W}?")]
    #[case::life(&["Pay 2 life"], "Pay 2 life?")]
    #[case::a_discard(&["Discard a card"], "Discard a card?")]
    #[case::mana_and_energy(&["Pay {1}", "Pay {E}{E}"], "Pay {1}, pay {E}{E}?")]
    #[case::three_parts(&["Pay {1}", "Pay 2 life", "Sacrifice a creature"], "Pay {1}, pay 2 life, sacrifice a creature?")]
    fn a_payment_is_offered_with_its_cost_as_printed(
        #[case] parts: &[&str],
        #[case] expected: &str,
    ) {
        let parts: Vec<String> = parts.iter().map(|part| (*part).to_owned()).collect();
        assert_eq!(pay(&parts), expected);
    }

    fn target(json: &str) -> String {
        spec(&serde_json::from_str(json).unwrap())
    }

    fn words(json: &str) -> String {
        pattern(&serde_json::from_str(json).unwrap())
    }

    #[rstest]
    #[case::one(1, "Discard a card.")]
    #[case::two(2, "Discard 2 cards.")]
    #[case::many(7, "Discard 7 cards.")]
    fn a_discard_asks_for_the_cards_it_takes(#[case] count: usize, #[case] expected: &str) {
        assert_eq!(discard(count), expected);
    }

    #[rstest]
    #[case::a_basic_land(
        "Library",
        r#"[{"Supertype":"Basic"},{"Type":"Land"}]"#,
        1,
        1,
        "Search your library for a basic land card."
    )]
    #[case::up_to_two(
        "Library",
        r#"[{"Supertype":"Basic"},{"Type":"Land"}]"#,
        0,
        2,
        "Search your library for up to 2 basic land cards."
    )]
    #[case::exactly_two(
        "Library",
        r#"[{"Supertype":"Basic"},{"Type":"Land"}]"#,
        2,
        2,
        "Search your library for 2 basic land cards."
    )]
    #[case::any_number(
        "Library",
        r#"[{"Type":"Creature"}]"#,
        0,
        usize::MAX,
        "Search your library for any number of creature cards."
    )]
    #[case::land_types(
        "Library",
        r#"[{"AnyOf":[{"Subtype":"Plains"},{"Subtype":"Island"}]}]"#,
        1,
        1,
        "Search your library for a Plains or Island card."
    )]
    #[case::a_supertype(
        "Library",
        r#"[{"Supertype":"Legendary"},{"Type":"Creature"}]"#,
        1,
        1,
        "Search your library for a legendary creature card."
    )]
    #[case::a_name(
        "Library",
        r#"[{"Named":"Wolf"}]"#,
        1,
        1,
        "Search your library for a card named Wolf."
    )]
    #[case::several_names(
        "Library",
        r#"[{"Named":"Wolf"}]"#,
        0,
        4,
        "Search your library for up to 4 cards named Wolf."
    )]
    #[case::any_card("Library", "[]", 1, 1, "Search your library for a card.")]
    #[case::the_zone_in_the_filter_is_the_zone_searched(
        "Library",
        r#"[{"In":"Library"},{"Type":"Land"}]"#,
        1,
        1,
        "Search your library for a land card."
    )]
    #[case::a_graveyard(
        "Graveyard",
        r#"[{"Type":"Creature"}]"#,
        1,
        1,
        "Search your graveyard for a creature card."
    )]
    #[case::a_hand(
        "Hand",
        r#"[{"Type":"Instant"}]"#,
        1,
        1,
        "Search your hand for an instant card."
    )]
    fn a_search_asks_for_the_cards_it_looks_for(
        #[case] zone: &str,
        #[case] filter: &str,
        #[case] min: usize,
        #[case] most: usize,
        #[case] expected: &str,
    ) {
        let zone = serde_json::from_str(&format!("\"{zone}\"")).unwrap();
        let filter = serde_json::from_str(filter).unwrap();
        assert_eq!(search(zone, &filter, min, most), expected);
    }

    #[rstest]
    #[case::two_kinds(
        r#"[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]"#,
        1,
        "Sacrifice a creature or planeswalker."
    )]
    #[case::a_creature(r#"[{"Type":"Creature"}]"#, 1, "Sacrifice a creature.")]
    #[case::yours(
        r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
        1,
        "Sacrifice a creature you control."
    )]
    #[case::an_artifact(r#"[{"Type":"Artifact"}]"#, 1, "Sacrifice an artifact.")]
    #[case::anything(r"[]", 1, "Sacrifice a permanent.")]
    #[case::itself(r#"["This"]"#, 1, "Sacrifice it.")]
    #[case::two_creatures(r#"[{"Type":"Creature"}]"#, 2, "Sacrifice 2 creatures.")]
    #[case::two_of_two_kinds(
        r#"[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]"#,
        2,
        "Sacrifice 2 creatures or planeswalkers."
    )]
    #[case::three_tapped_lands(
        r#"[{"Type":"Land"},{"Tapped":null}]"#,
        3,
        "Sacrifice 3 tapped lands."
    )]
    fn a_sacrifice_asks_for_the_permanents_it_takes(
        #[case] filter: &str,
        #[case] count: usize,
        #[case] expected: &str,
    ) {
        assert_eq!(
            sacrifice(&serde_json::from_str(filter).unwrap(), count),
            expected
        );
    }

    #[rstest]
    #[case::any_target(r#"{"what":"AnyTarget"}"#, "any target")]
    #[case::a_player(r#"{"what":"Player"}"#, "target player")]
    #[case::an_opponent(r#"{"what":"Opponent"}"#, "target opponent")]
    #[case::a_creature(r#"{"what":{"Object":[{"Type":"Creature"}]}}"#, "target creature")]
    #[case::yours(
        r#"{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
        "target creature you control"
    )]
    #[case::a_card_in_your_graveyard(
        r#"{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}"#,
        "target card in your graveyard"
    )]
    #[case::coloured_and_tapped(
        r#"{"what":{"Object":[{"Color":"White"},{"Tapped":null},{"Type":"Creature"}]}}"#,
        "target white tapped creature"
    )]
    #[case::small(
        r#"{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":2}}]}}"#,
        "target creature with power 2 or less"
    )]
    #[case::two_kinds(
        r#"{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]}}"#,
        "target creature or planeswalker"
    )]
    #[case::with_a_counter(
        r#"{"what":{"Object":[{"Type":"Creature"},{"HasCounter":"+1/+1"}]}}"#,
        "target creature with a +1/+1 counter on it"
    )]
    #[case::with_a_named_counter_you_control(
        r#"{"what":{"Object":[{"Type":"Enchantment"},{"ControlledBy":"You"},{"HasCounter":{"Other":"quest"}}]}}"#,
        "target enchantment you control with a quest counter on it"
    )]
    #[case::up_to_one(
        r#"{"what":"Player","bounds":{"min":0,"max":1}}"#,
        "up to one target player"
    )]
    #[case::up_to_two(
        r#"{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}"#,
        "up to 2 target creatures"
    )]
    #[case::exactly_two(r#"{"what":"Player","bounds":{"min":2,"max":2}}"#, "2 target players")]
    #[case::any_number(
        r#"{"what":{"Object":[{"Type":"Artifact"}]},"bounds":{"min":0,"max":null}}"#,
        "any number of target artifacts"
    )]
    #[case::either(
        r#"{"what":{"Either":["Player",{"Object":[{"Type":"Creature"}]}]}}"#,
        "target player or creature"
    )]
    fn a_target_specification_reads_as_a_card_says_it(#[case] json: &str, #[case] expected: &str) {
        assert_eq!(target(json), expected);
    }

    #[rstest]
    #[case::enters(
        r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#,
        "When it enters the battlefield"
    )]
    #[case::dies(
        r#"{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}"#,
        "When it dies"
    )]
    #[case::another_dies(
        r#"{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}}"#,
        "Whenever a creature you control dies"
    )]
    #[case::lands(
        r#"{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}}"#,
        "Whenever a land you control enters the battlefield"
    )]
    #[case::attacks(r#"{"Attacks":{"what":["This"]}}"#, "When it attacks")]
    #[case::opponent_casts(
        r#"{"SpellCast":{"by":"Opponents"}}"#,
        "Whenever an opponent casts a spell"
    )]
    #[case::you_cast(
        r#"{"SpellCast":{"what":[{"Type":"Instant"}],"by":"You"}}"#,
        "Whenever you cast an instant spell"
    )]
    #[case::sacrificed(
        r#"{"Sacrificed":{"what":[{"Subtype":"Clue"}],"by":"You"}}"#,
        "Whenever you sacrifice a Clue"
    )]
    #[case::upkeep(
        r#"{"StepBegins":{"step":"Upkeep","whose":"You"}}"#,
        "At the beginning of your upkeep"
    )]
    #[case::any_end_step(
        r#"{"StepBegins":{"step":"End"}}"#,
        "At the beginning of each end step"
    )]
    #[case::combat(
        r#"{"StepBegins":{"step":"BeginCombat","whose":"Opponents"}}"#,
        "At the beginning of combat on an opponent's turn"
    )]
    #[case::target(
        r#"{"BecomesTarget":{"what":["This"]}}"#,
        "When it becomes the target of a spell or ability"
    )]
    #[case::target_of_you(
        r#"{"BecomesTarget":{"what":["This"],"by":[{"ControlledBy":"You"}]}}"#,
        "When it becomes the target of a spell or ability you control"
    )]
    #[case::target_of_an_opponent(
        r#"{"BecomesTarget":{"what":["This"],"by":[{"ControlledBy":"Opponents"}]}}"#,
        "When it becomes the target of a spell or ability an opponent controls"
    )]
    #[case::either(
        r#"{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]}"#,
        "When it enters the battlefield or it attacks"
    )]
    #[case::no_islands(
        r#"{"State":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Island"},{"ControlledBy":"You"}]},"op":"Eq","right":0}}}}"#,
        "When you control no Islands"
    )]
    #[case::no_other_creatures(
        r#"{"State":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"op":"Eq","right":0}}}}"#,
        "When you control no other creatures"
    )]
    #[case::no_cards_in_hand(
        r#"{"State":{"condition":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Eq","right":0}}}}"#,
        "When you have no cards in hand"
    )]
    #[case::no_creatures_at_all(
        r#"{"State":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"}]},"op":"Eq","right":0}}}}"#,
        "When there are no creatures"
    )]
    #[case::little_life(
        r#"{"State":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Le","right":10}}}}"#,
        "When you have 10 or less life"
    )]
    #[case::a_state_with_no_phrase(
        r#"{"State":{"condition":"YourTurn"}}"#,
        "When its state is true"
    )]
    #[case::combat_damage(
        r#"{"DealsDamage":{"source":["This"],"combat":true}}"#,
        "When it deals combat damage"
    )]
    #[case::damage_by_a_creature(
        r#"{"DealsDamage":{"source":[{"Type":"Creature"}]}}"#,
        "Whenever a creature deals damage"
    )]
    #[case::dealt_damage(
        r#"{"DealsDamage":{"source":[],"to":"This"}}"#,
        "When it is dealt damage"
    )]
    #[case::a_creature_dealt_damage(
        r#"{"DealsDamage":{"source":[],"to":{"Each":[{"Type":"Creature"}]}}}"#,
        "Whenever a creature is dealt damage"
    )]
    #[case::nth_target(
        r#"{"Nth":{"event":{"BecomesTarget":{"what":["This"],"by":[{"ControlledBy":"You"}]}},"nth":1}}"#,
        "Whenever it becomes the target of a spell or ability you control (time 1 this turn)"
    )]
    #[case::nth_attack(
        r#"{"Nth":{"event":{"Attacks":{"what":["This"]}},"nth":2}}"#,
        "Whenever it attacks (time 2 this turn)"
    )]
    #[case::nth_damage(
        r#"{"Nth":{"event":{"DealsDamage":{"source":[],"to":"This"}},"nth":1}}"#,
        "Whenever it is dealt damage (time 1 this turn)"
    )]
    #[case::noncombat_damage(
        r#"{"DealsDamage":{"source":[{"Type":"Creature"}],"combat":false}}"#,
        "Whenever a creature deals noncombat damage"
    )]
    #[case::you_gain_life(r#"{"LifeGained":{"who":"You"}}"#, "Whenever you gain life")]
    #[case::a_player_gains_life(r#"{"LifeGained":{}}"#, "Whenever a player gains life")]
    #[case::an_opponent_loses_life(
        r#"{"LifeLost":{"who":"Opponents"}}"#,
        "Whenever an opponent loses life"
    )]
    #[case::you_draw(r#"{"Drew":{"who":"You"}}"#, "Whenever you draw a card")]
    #[case::you_discard(r#"{"Discarded":{"by":"You"}}"#, "Whenever you discard a card")]
    #[case::you_discard_a_land(
        r#"{"Discarded":{"what":[{"Type":"Land"}],"by":"You"}}"#,
        "Whenever you discard a land"
    )]
    #[case::becoming_tapped(r#"{"Tapped":{"what":["This"]}}"#, "When it becomes tapped")]
    #[case::tapping_a_land_for_mana(
        r#"{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}}"#,
        "Whenever you tap a land for mana"
    )]
    #[case::blocking(r#"{"Blocks":{"what":["This"]}}"#, "When it blocks")]
    #[case::becoming_blocked(r#"{"BecomesBlocked":{"what":["This"]}}"#, "When it becomes blocked")]
    #[case::attacking_together(
        r#"{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at_least":2}}"#,
        "Whenever 2 or more creatures you control attack"
    )]
    #[case::counters_put(
        r#"{"CountersPut":{"what":[{"Type":"Creature"}]}}"#,
        "Whenever counters are put on a creature"
    )]
    #[case::untapped(
        r#"{"Untapped":{"what":[{"Type":"Creature"}]}}"#,
        "Whenever a creature becomes untapped"
    )]
    #[case::this_untaps(r#"{"Untapped":{"what":["This"]}}"#, "When it becomes untapped")]
    #[case::counters_removed(
        r#"{"CountersRemoved":{"what":["This"]}}"#,
        "Whenever counters are removed from it"
    )]
    #[case::a_chapter(
        r#"{"Chapter":{"number":3}}"#,
        "When its lore counters reach chapter 3"
    )]
    #[case::counters_removed_anywhere(
        r#"{"CountersRemoved":{}}"#,
        "Whenever counters are removed from a permanent"
    )]
    #[case::an_aura_attached_to_this(
        r#"{"Attached":{"what":[{"Subtype":"Aura"}],"to":["This"]}}"#,
        "Whenever an Aura becomes attached to it"
    )]
    #[case::this_attached(
        r#"{"Attached":{"what":["This"],"to":[{"Type":"Creature"}]}}"#,
        "When it becomes attached to a creature"
    )]
    #[case::you_cycle(r#"{"Cycled":{"by":"You"}}"#, "Whenever you cycle a card")]
    #[case::a_player_cycles(r#"{"Cycled":{}}"#, "Whenever a player cycles a card")]
    #[case::you_cycle_this(r#"{"Cycled":{"what":["This"],"by":"You"}}"#, "When you cycle it")]
    #[case::you_become_the_monarch(
        r#"{"BecomesMonarch":{"who":"You"}}"#,
        "Whenever you become the monarch"
    )]
    #[case::an_opponent_becomes_the_monarch(
        r#"{"BecomesMonarch":{"who":"Opponents"}}"#,
        "Whenever an opponent becomes the monarch"
    )]
    #[case::day_or_night_changes(
        r#"{"DayNightChanges":{}}"#,
        "Whenever day becomes night or night becomes day"
    )]
    #[case::night_becomes_day(r#"{"DayNightChanges":{"to":"Day"}}"#, "Whenever night becomes day")]
    #[case::day_becomes_night(
        r#"{"DayNightChanges":{"to":"Night"}}"#,
        "Whenever day becomes night"
    )]
    #[case::you_proliferate(r#"{"Proliferated":{"who":"You"}}"#, "Whenever you proliferate")]
    #[case::you_amass(r#"{"Amassed":{"who":"You"}}"#, "Whenever you amass")]
    #[case::you_bolster(r#"{"Bolstered":{"who":"You"}}"#, "Whenever you bolster")]
    #[case::an_opponent_bolsters(
        r#"{"Bolstered":{"who":"Opponents"}}"#,
        "Whenever an opponent bolsters"
    )]
    #[case::this_becomes_monstrous(
        r#"{"BecomesMonstrous":{"what":["This"]}}"#,
        "When it becomes monstrous"
    )]
    #[case::this_vehicle_becomes_crewed(
        r#"{"BecomesCrewed":{"what":["This"]}}"#,
        "When it becomes crewed"
    )]
    #[case::a_vehicle_you_control_becomes_crewed(
        r#"{"BecomesCrewed":{"what":[{"ControlledBy":"You"}]}}"#,
        "Whenever a permanent you control becomes crewed"
    )]
    #[case::this_mount_becomes_saddled(
        r#"{"BecomesSaddled":{"what":["This"]}}"#,
        "When it becomes saddled"
    )]
    #[case::the_first_crew_each_turn(
        r#"{"Nth":{"event":{"BecomesCrewed":{"what":["This"]}},"nth":1}}"#,
        "Whenever it becomes crewed (time 1 this turn)"
    )]
    #[case::this_class_becomes_level_two(
        r#"{"BecomesLevel":{"what":["This"],"level":2}}"#,
        "When it becomes level 2"
    )]
    #[case::a_creature_adapts(
        r#"{"Adapted":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
        "Whenever a creature you control adapts"
    )]
    #[case::an_opponent_amasses(
        r#"{"Amassed":{"who":"Opponents"}}"#,
        "Whenever an opponent amasses"
    )]
    #[case::a_player_becomes_the_monarch(
        r#"{"BecomesMonarch":{}}"#,
        "Whenever a player becomes the monarch"
    )]
    #[case::attacking_the_monarch(
        r#"{"AttacksPlayer":{"what":[{"Type":"Creature"}],"at":"Monarch"}}"#,
        "Whenever a creature attacks the monarch"
    )]
    #[case::attacking_the_player_with_the_most_life(
        r#"{"AttacksPlayer":{"what":["This"],"at":"MostLife"}}"#,
        "Whenever it attacks the player with the most life"
    )]
    #[case::attacking_alone(
        r#"{"AttacksAlone":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
        "Whenever a creature you control attacks alone"
    )]
    #[case::the_monarchs_end_step(
        r#"{"StepBegins":{"step":"End","whose":"Monarch"}}"#,
        "At the beginning of the monarch's end step"
    )]
    fn a_trigger_pattern_reads_as_a_card_says_it(#[case] json: &str, #[case] expected: &str) {
        assert_eq!(words(json), expected);
    }

    #[rstest]
    #[case::attacks("Battle cry", "Whenever it attacks", "Battle cry: whenever it attacks")]
    #[case::cast(
        "Prowess",
        "Whenever you cast a noncreature spell",
        "Prowess: whenever you cast a noncreature spell"
    )]
    #[case::nothing("Ward", "", "Ward: ")]
    fn a_keywords_trigger_is_told_by_the_keyword(
        #[case] keyword: &str,
        #[case] said: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(keyword_trigger(keyword, said), expected);
    }

    fn may_words(json: &str) -> String {
        may(&serde_json::from_str::<Vec<Effect>>(json).unwrap())
    }

    #[rstest]
    #[case::draw("[{\"Draw\":{}}]", "Draw a card?")]
    #[case::draw_more("[{\"Draw\":{\"count\":3}}]", "Draw 3 cards?")]
    #[case::discard("[{\"Discard\":{}}]", "Discard a card?")]
    #[case::mill("[{\"Mill\":{\"count\":2}}]", "Mill 2 cards?")]
    #[case::scry("[{\"Scry\":{\"count\":2}}]", "Scry 2?")]
    #[case::life("[{\"GainLife\":{\"amount\":4}}]", "Gain 4 life?")]
    #[case::lose("[{\"LoseLife\":{\"amount\":1}}]", "Lose 1 life?")]
    #[case::two(
        "[{\"Draw\":{}},{\"GainLife\":{\"amount\":1}}]",
        "Draw a card and gain 1 life?"
    )]
    #[case::three(
        "[{\"Draw\":{}},{\"Discard\":{}},{\"LoseLife\":{\"amount\":1}}]",
        "Draw a card, discard a card and lose 1 life?"
    )]
    #[case::done_to_another("[{\"Draw\":{\"who\":\"Opponents\"}}]", "")]
    #[case::a_random_discard("[{\"Discard\":{\"random\":true}}]", "")]
    #[case::an_amount_that_is_not_a_number("[{\"Draw\":{\"count\":\"X\"}}]", "")]
    #[case::one_effect_without_words("[{\"Draw\":{}},{\"Untap\":{\"what\":\"This\"}}]", "")]
    #[case::no_effect("[]", "")]
    fn a_may_asks_what_its_effects_do_or_nothing(#[case] json: &str, #[case] expected: &str) {
        assert_eq!(may_words(json), expected);
    }

    #[rstest]
    #[case::opponents(r#""Opponents""#, "Choose an opponent.")]
    #[case::everyone(r#""EachPlayer""#, "Choose a player.")]
    #[case::a_target(r#"{"Target":0}"#, "Choose a player.")]
    fn a_choice_among_players_asks_for_one(#[case] from: &str, #[case] expected: &str) {
        assert_eq!(
            choose_player(&serde_json::from_str(from).unwrap()),
            expected
        );
    }

    fn objects(from: &str, min: usize, most: usize) -> String {
        choose_objects(&serde_json::from_str(from).unwrap(), min, most)
    }

    #[rstest]
    #[case::a_card_in_hand(
        r#"{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]}"#,
        1,
        1,
        "Choose a card in your hand."
    )]
    #[case::two_cards(
        r#"{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]}"#,
        2,
        2,
        "Choose 2 cards in your hand."
    )]
    #[case::up_to_two(
        r#"{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]}"#,
        0,
        2,
        "Choose up to 2 cards in your hand."
    )]
    #[case::a_creature(
        r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
        1,
        1,
        "Choose a creature you control."
    )]
    #[case::of_what_was_found(r#"{"Bound":"found"}"#, 1, 1, "Choose a card.")]
    #[case::some_of_what_was_found(r#"{"Bound":"found"}"#, 2, 2, "Choose 2 cards.")]
    fn a_choice_of_objects_asks_for_what_it_takes(
        #[case] from: &str,
        #[case] min: usize,
        #[case] most: usize,
        #[case] expected: &str,
    ) {
        assert_eq!(objects(from, min, most), expected);
    }
}
