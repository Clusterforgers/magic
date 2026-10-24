//! Triggers: the event a triggered ability waits for, and the condition of an intervening "if",
//! read from "when ~ enters", "whenever you cast a creature spell" or "at the beginning of your
//! upkeep" and written back the same way.

use super::amounts::{count_word, number_word};
use super::batches;
use super::counters;
use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::keyword::Keyword;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{Step, TriggerPattern};
use crate::value::{Amount, CastWay, Cmp, Condition, CounterKind};
use mtg_core::card::CardType;

/// Whether the pattern is "when you cycle this card", the one trigger that works from the graveyard.
pub fn cycled_itself(pattern: &TriggerPattern) -> bool {
    matches!(
        pattern,
        TriggerPattern::Cycled { what, by: Ref::You } if what.0 == [Pred::This]
    )
}

/// Whether the pattern is the card itself being cast by its controller ("When you cast this spell"), so its ability works
/// from the stack.
pub fn cast_itself(pattern: &TriggerPattern) -> bool {
    matches!(
        pattern,
        TriggerPattern::SpellCast { what, by: Ref::You } if what.0 == [Pred::This]
    )
}

/// Whether the pattern is the card itself put into a graveyard from anywhere, so its ability works from the graveyard.
pub fn graveyard_itself(pattern: &TriggerPattern) -> bool {
    matches!(
        pattern,
        TriggerPattern::ZoneChange { what, from: None, to: Some(ZoneKind::Graveyard) } if what.0 == [Pred::This]
    )
}

/// Whether the pattern is the card itself entering, attacking or dealing damage, so the card's keyword actions may call it
/// "it".
pub fn itself_acts(pattern: &TriggerPattern) -> bool {
    match pattern {
        TriggerPattern::Attacks { what }
        | TriggerPattern::ZoneChange {
            what,
            to: Some(ZoneKind::Battlefield),
            ..
        } => what.0 == [Pred::This],
        // "Whenever ~ is dealt damage, it fights up to one target creature" (enrage).
        TriggerPattern::DealsDamage { source, to, .. } => {
            source.0 == [Pred::This] || *to == Some(Ref::This)
        }
        TriggerPattern::AnyOf(patterns) => patterns.iter().all(itself_acts),
        _ => false,
    }
}

/// What "it" means in the effects of a trigger about one creature: the card itself when it attacks, blocks, becomes
/// blocked or enters, and the creature that attacks, becomes blocked or enters otherwise. A creature that blocks
/// shares the cause with the creature it blocks, so another blocker has no "it".
pub fn event_creature(pattern: &TriggerPattern) -> Option<Ref> {
    let (what, named_by_the_cause) = match pattern {
        TriggerPattern::Attacks { what }
        | TriggerPattern::BecomesBlocked { what }
        | TriggerPattern::Mutates { what }
        | TriggerPattern::ZoneChange {
            what,
            to: Some(ZoneKind::Battlefield),
            ..
        } => (what, true),
        TriggerPattern::Blocks { what } => (what, false),
        // The card that dies, or is put into a graveyard, is "it" in what the card does about itself.
        TriggerPattern::ZoneChange {
            what,
            to: Some(ZoneKind::Graveyard),
            ..
        } if what.0 == [Pred::This] => return Some(Ref::This),
        // "Whenever ~ or another creature you control enters": the permanent that entered, the card included.
        TriggerPattern::AnyOf(patterns)
            if patterns.iter().all(|pattern| {
                matches!(
                    pattern,
                    TriggerPattern::ZoneChange {
                        to: Some(ZoneKind::Battlefield),
                        ..
                    }
                )
            }) =>
        {
            return Some(Ref::Triggering);
        }
        // "Whenever this creature or equipped creature attacks": the creature that attacks.
        TriggerPattern::AnyOf(patterns)
            if patterns
                .iter()
                .all(|pattern| matches!(pattern, TriggerPattern::Attacks { .. })) =>
        {
            return Some(Ref::Triggering);
        }
        _ => return None,
    };
    if what.0 == [Pred::This] {
        Some(Ref::This)
    } else {
        named_by_the_cause.then_some(Ref::Triggering)
    }
}

/// The object a trigger's event is about, which a where clause calls "its" or "that creature": the card for an event of
/// its own, the creature of the event otherwise.
pub fn event_subject(pattern: &TriggerPattern) -> Option<Ref> {
    let (TriggerPattern::Attacks { what }
    | TriggerPattern::BecomesBlocked { what }
    | TriggerPattern::Blocks { what }
    | TriggerPattern::ZoneChange { what, .. }) = pattern
    else {
        return None;
    };
    Some(if what.0 == [Pred::This] {
        Ref::This
    } else {
        Ref::Triggering
    })
}

/// Whether the pattern is the card itself becoming a target (counted or not) or dealing damage to a player, so the card's
/// effects may call it "it".
pub fn targeted_itself(pattern: &TriggerPattern) -> bool {
    match pattern {
        TriggerPattern::Nth { event, .. } => targeted_itself(event),
        TriggerPattern::BecomesTarget { what, .. } => what.0 == [Pred::This],
        TriggerPattern::DealsDamage {
            source,
            to: Some(_),
            ..
        } => source.0 == [Pred::This],
        _ => false,
    }
}

/// Whether the pattern is the card itself dying or leaving the battlefield, so the card's damage may say "it deals".
pub fn leaves_itself(pattern: &TriggerPattern) -> bool {
    matches!(
        pattern,
        TriggerPattern::ZoneChange {
            what,
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard) | None,
        } if what.0 == [Pred::This]
    )
}

/// The steps a trigger names and the words for each.
const STEPS: [(Step, &str); 5] = [
    (Step::Upkeep, "upkeep"),
    (Step::Draw, "draw step"),
    (Step::End, "end step"),
    (Step::Main1, "precombat main phase"),
    (Step::Main2, "postcombat main phase"),
];

/// The newer wordings of the main phases: the card text says "first" and "second" where the rules
/// said "precombat" and "postcombat". Both are read; the older is written.
const NEWER_STEPS: [(Step, &str); 2] = [
    (Step::Main1, "first main phase"),
    (Step::Main2, "second main phase"),
];

/// The event of a trigger, from its first word: "When ~ enters", "Whenever you cast a spell",
/// "At the beginning of your upkeep".
pub fn pattern(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    if r.words("at the beginning of") {
        return step(r);
    }
    if !(r.word("when") || r.word("whenever")) {
        return None;
    }
    if let Some(found) = r.attempt(player_event) {
        return Some(found);
    }
    if let Some(found) = r.attempt(super::day_night::changes) {
        return Some(found);
    }
    if let Some(found) = r.attempt(attached_tapped_for_mana) {
        return Some(found);
    }
    if let Some(found) = r.attempt(counter_event) {
        return Some(found);
    }
    if let Some(found) = r.attempt(together) {
        return Some(found);
    }
    let what = subject(r)?;
    if let Some(found) = r.attempt(|r| with_another(r, &what)) {
        return Some(found);
    }
    verbs(r, &what)
}

/// "enchanted land is tapped for mana" (also "enchanted Forest"): the land an Aura is attached to, tapped by any player.
fn attached_tapped_for_mana(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    r.word("enchanted").then_some(())?;
    let land = phrase(r, Form::Subject)?;
    r.need("is tapped for mana")?;
    let mut what = vec![Pred::Is(Ref::Attached)];
    what.extend(land.0);
    Some(TriggerPattern::TappedForMana {
        what: Selector(what),
        by: Ref::EachPlayer,
    })
}

/// The card itself and a second subject doing the same: "~ or another Elf you control enters", "this creature or
/// another creature dies or deals combat damage to a player", "this creature or equipped creature attacks". The events
/// are the verbs of the card and then of the second subject, one verb at a time.
fn with_another(r: &mut Reader<'_>, first: &Selector) -> Option<TriggerPattern> {
    (*first == Selector::this()).then_some(())?;
    r.word("or").then_some(())?;
    let second = subject(r)?;
    (second.0.contains(&Pred::Other) || second.0 == [Pred::Is(Ref::Attached)]).then_some(())?;
    // The verbs are read twice, once for each subject.
    let firsts = r.lookahead(|r| verb_list(r, first))?;
    let seconds = verb_list(r, &second)?;
    // A count of one event is not part of a list of events.
    let counted = |found: &[TriggerPattern]| {
        found
            .iter()
            .any(|found| matches!(found, TriggerPattern::Nth { .. }))
    };
    (firsts.len() == seconds.len() && !counted(&firsts) && !counted(&seconds)).then_some(())?;
    let events: Vec<TriggerPattern> = firsts
        .into_iter()
        .zip(seconds)
        .flat_map(|(itself, other)| [itself, other])
        .collect();
    Some(TriggerPattern::AnyOf(events))
}

/// The events that happen together: "one or more creatures you control deal combat damage to a player", "one or more
/// cards are put into your graveyard from your library", "one or more creatures attack".
fn together(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    let at_least = if r.words("one or more") {
        1
    } else {
        let number = number_word(r.take_word()?)?;
        r.need("or more")?;
        number
    };
    if at_least == 1
        && let Some(found) = r.attempt(counters_batch)
    {
        return Some(found);
    }
    if at_least == 1 && r.words("cards are put into your graveyard from your library") {
        let what = Selector(vec![Pred::OwnedBy(Ref::You)]);
        return Some(TriggerPattern::Batch(Box::new(
            TriggerPattern::ZoneChange {
                what,
                from: Some(ZoneKind::Library),
                to: Some(ZoneKind::Graveyard),
            },
        )));
    }
    if at_least == 1 && r.words("cards leave your graveyard") {
        return Some(TriggerPattern::Batch(Box::new(
            TriggerPattern::ZoneChange {
                what: Selector(vec![Pred::OwnedBy(Ref::You)]),
                from: Some(ZoneKind::Graveyard),
                to: None,
            },
        )));
    }
    let what = phrase(r, Form::All)?;
    if r.word("attack") {
        return Some(TriggerPattern::GroupAttacks { what, at_least });
    }
    if at_least != 1 || !r.word("deal") {
        return None;
    }
    let damage = damage_dealt(r, &what)?;
    Some(TriggerPattern::Batch(Box::new(damage)))
}

/// "+1/+1 counters are put on this creature", "counters are put on a creature you control", after "one or more": the
/// counters of one batch.
fn counters_batch(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    let counter = if r.words("counters are put on") {
        None
    } else {
        let counter = counters::kind(r)?;
        r.words("counters are put on").then_some(())?;
        Some(counter)
    };
    let what = subject(r)?;
    let put = TriggerPattern::CountersPut {
        what,
        counter: counter.clone(),
    };
    // The counters of any kind put on the object in a turn are counted.
    if counter.is_none() {
        let timed = first_time(r, put.clone());
        if matches!(timed, TriggerPattern::Nth { .. }) {
            return Some(timed);
        }
    }
    Some(TriggerPattern::Batch(Box::new(put)))
}

/// "a +1/+1 counter is put on ~", "a counter is removed from a creature you control".
fn counter_event(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    (r.word("a") || r.word("an")).then_some(())?;
    // A plain "counter" is a counter of any kind.
    let counter = if r.word("counter") {
        None
    } else {
        Some(counters::trigger_kind(r)?)
    };
    let putting = if r.words("is put on") {
        true
    } else if r.words("is removed from") {
        false
    } else {
        return None;
    };
    let what = subject(r)?;
    Some(if putting {
        TriggerPattern::CountersPut { what, counter }
    } else {
        TriggerPattern::CountersRemoved { what, counter }
    })
}

/// "your upkeep", "each end step", "each opponent's draw step", "combat on your turn", "each combat".
fn step(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    let combat = |whose| TriggerPattern::StepBegins {
        step: Step::BeginCombat,
        whose,
    };
    if r.words("combat on your turn") {
        return Some(combat(Ref::You));
    }
    if r.words("each combat") {
        return Some(combat(Ref::EachPlayer));
    }
    let whose = if r.word("your") {
        Ref::You
    } else if r.words("the monarch's") {
        Ref::Monarch
    } else if r.words("each opponent's") {
        Ref::Opponents
    } else if r.words("each player's") || r.word("each") {
        Ref::EachPlayer
    } else {
        return None;
    };
    let (step, _) = STEPS
        .iter()
        .chain(&NEWER_STEPS)
        .find(|(_, words)| r.words(words))?;
    Some(TriggerPattern::StepBegins { step: *step, whose })
}

/// The one doing it: "you", "an opponent" or "a player", and whether the verb takes an "s".
fn player(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    if r.word("you") {
        Some((Ref::You, false))
    } else if r.words("an opponent") {
        Some((Ref::Opponents, true))
    } else if r.words("a player") {
        Some((Ref::EachPlayer, true))
    } else if r.words("enchanted player") {
        Some((Ref::AttachedPlayer, true))
    } else {
        None
    }
}

/// What follows "you cast" or "an opponent casts": the nth spell of a turn, the card itself ("this spell", on the stack)
/// or a kind of spell.
fn spell_cast(r: &mut Reader<'_>, by: Ref, third: bool) -> Option<TriggerPattern> {
    if let Some(nth) = each_turn(r, "spell", if third { "their" } else { "your" }) {
        let event = TriggerPattern::SpellCast {
            what: Selector::default(),
            by,
        };
        return Some(TriggerPattern::Nth {
            event: Box::new(event),
            nth,
        });
    }
    if !third && r.source() {
        return Some(TriggerPattern::SpellCast {
            what: Selector::this(),
            by,
        });
    }
    let mut spell = subject(r)?.0;
    // The pattern is about a spell already; the selector keeps what the spell must be.
    if spell.first() != Some(&Pred::Spell) || spell.contains(&Pred::Other) {
        return None;
    }
    spell.remove(0);
    Some(TriggerPattern::SpellCast {
        what: Selector(spell),
        by,
    })
}

/// "you cast a creature spell", "an opponent casts a spell", "you sacrifice a Clue".
fn player_event(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    let (by, third) = player(r)?;
    let verb = r.take_word()?;
    let whose = if third { "their" } else { "your" };
    if verb == if third { "casts" } else { "cast" } {
        spell_cast(r, by, third)
    } else if verb == if third { "sacrifices" } else { "sacrifice" } {
        Some(TriggerPattern::Sacrificed {
            what: subject(r)?,
            by,
        })
    } else if verb == if third { "gains" } else { "gain" } {
        r.need("life")?;
        Some(first_time(r, TriggerPattern::LifeGained { who: by }))
    } else if verb == if third { "loses" } else { "lose" } {
        r.need("life")?;
        Some(first_time(r, TriggerPattern::LifeLost { who: by }))
    } else if verb == if third { "draws" } else { "draw" } {
        if let Some(nth) = each_turn(r, "card", whose) {
            let event = TriggerPattern::Drew {
                who: by,
                what: Selector::default(),
            };
            return Some(TriggerPattern::Nth {
                event: Box::new(event),
                nth,
            });
        }
        r.need("a card")?;
        Some(TriggerPattern::Drew {
            who: by,
            what: Selector::default(),
        })
    } else if verb == if third { "discards" } else { "discard" } {
        r.need("a card")?;
        Some(TriggerPattern::Discarded {
            what: Selector::default(),
            by,
        })
    } else if verb == if third { "cycles" } else { "cycle" } {
        let what = if r.words("another card") {
            Selector(vec![Pred::Other])
        } else if r.source() {
            Selector::this()
        } else {
            r.need("a card")?;
            Selector::default()
        };
        // The card itself is cycled once, so it has no count.
        let itself = what == Selector::this();
        let cycled = TriggerPattern::Cycled { what, by };
        Some(if itself {
            cycled
        } else {
            first_time(r, cycled)
        })
    } else if verb == if third { "surveils" } else { "surveil" } {
        Some(first_time(r, TriggerPattern::Surveilled { who: by }))
    } else if verb == if third { "creates" } else { "create" } {
        tokens_created(r, &by)
    } else if verb == if third { "becomes" } else { "become" } {
        r.need("the monarch")?;
        Some(TriggerPattern::BecomesMonarch { who: by })
    } else if verb == if third { "is" } else { "are" } {
        r.need("attacked")?;
        Some(TriggerPattern::PlayerAttacked { who: by })
    } else if verb == if third { "attacks" } else { "attack" } {
        if by == Ref::You && r.peek_word() == Some("with") {
            return attack_with(r);
        }
        let at = if r.words("enchanted player") {
            Ref::AttachedPlayer
        } else {
            r.word("you").then_some(Ref::You)?
        };
        r.need("with one or more creatures")?;
        Some(TriggerPattern::PlayerAttacks { by, at })
    } else if verb == if third { "proliferates" } else { "proliferate" } {
        Some(TriggerPattern::Proliferated { who: by })
    } else if verb == if third { "bolsters" } else { "bolster" } {
        Some(TriggerPattern::Bolstered { who: by })
    } else if verb == if third { "amasses" } else { "amass" } {
        Some(TriggerPattern::Amassed { who: by })
    } else if verb == if third { "taps" } else { "tap" } {
        let what = subject(r)?;
        r.need("for mana")?;
        Some(TriggerPattern::TappedForMana { what, by })
    } else {
        None
    }
}

/// "you create one or more tokens for the first time each turn": the first token of the turn that enters under your
/// control. Only the count of the turn has a pattern; the event of every token is a token entering.
fn tokens_created(r: &mut Reader<'_>, by: &Ref) -> Option<TriggerPattern> {
    (*by == Ref::You && r.words("one or more tokens")).then_some(())?;
    let created = TriggerPattern::ZoneChange {
        what: Selector(vec![Pred::Token, Pred::ControlledBy(Ref::You)]),
        from: None,
        to: Some(ZoneKind::Battlefield),
    };
    let counted = first_time(r, created);
    matches!(counted, TriggerPattern::Nth { .. }).then_some(counted)
}

/// "you attack with two or more creatures": that many of the creatures you control attack together.
fn attack_with(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    r.need("with")?;
    let at_least = if r.words("one or more") {
        1
    } else {
        let number = number_word(r.take_word()?)?;
        r.need("or more")?;
        number
    };
    r.need("creatures")?;
    Some(TriggerPattern::GroupAttacks {
        what: Selector(vec![
            Pred::Type(CardType::Creature),
            Pred::ControlledBy(Ref::You),
        ]),
        at_least,
    })
}

/// "your second card each turn", "their first spell each turn": which one of the turn, for a noun.
fn each_turn(r: &mut Reader<'_>, noun: &str, whose: &str) -> Option<u32> {
    r.attempt(|r| {
        r.need(whose)?;
        let at = ORDINALS.iter().position(|word| r.word(word))?;
        r.need(noun)?;
        r.need("each turn")?;
        u32::try_from(at + 1).ok()
    })
}

/// The ordinals a count of a turn names.
const ORDINALS: [&str; 5] = ["first", "second", "third", "fourth", "fifth"];

/// The card itself, or an object with its article: "a creature you control", "another creature".
fn subject(r: &mut Reader<'_>) -> Option<Selector> {
    if r.source() {
        return Some(Selector::this());
    }
    // What an Equipment or an Aura is attached to.
    let attached = r.attempt(|r| {
        (r.word("equipped") || r.word("enchanted")).then_some(())?;
        r.word("creature").then_some(())
    });
    if attached.is_some() {
        return Some(Selector(vec![Pred::Is(Ref::Attached)]));
    }
    r.attempt(|r| {
        let article = r.word("a") || r.word("an");
        let selector = phrase(r, Form::Subject)?;
        // "another" stands for the article and for "other", and "your commander" needs none.
        let bare = selector.0.contains(&Pred::Other) || own_commander(&selector);
        (article != bare).then_some(selector)
    })
}

/// What the subject does: "enters", "dies", "leaves the battlefield", "attacks", and several of them
/// joined by "or".
fn verbs(r: &mut Reader<'_>, what: &Selector) -> Option<TriggerPattern> {
    let mut found = verb_list(r, what)?;
    if found.len() == 1 {
        found.pop()
    } else if found
        .iter()
        .any(|found| matches!(found, TriggerPattern::Nth { .. }))
    {
        // A count of one event is not part of a list of events.
        None
    } else {
        Some(TriggerPattern::AnyOf(found))
    }
}

/// The events of the verbs the subject does, in the order they are written.
fn verb_list(r: &mut Reader<'_>, what: &Selector) -> Option<Vec<TriggerPattern>> {
    let mut found = vec![verb(r, what)?];
    while r.word("or") {
        let next = verb(r, what)?;
        if found.contains(&next) {
            return None;
        }
        found.push(next);
    }
    Some(found)
}

/// "is put into a graveyard from anywhere other than the battlefield", also "into your graveyard" (the cards it owns).
fn from_elsewhere(r: &mut Reader<'_>, what: &Selector) -> Option<TriggerPattern> {
    r.need("is put into")?;
    let mut picked = what.clone();
    if r.words("your graveyard") {
        picked.0.push(Pred::OwnedBy(Ref::You));
    } else {
        r.need("a graveyard")?;
    }
    r.need("from anywhere other than the battlefield")?;
    Some(TriggerPattern::ZoneChangeExcept {
        what: picked,
        except: ZoneKind::Battlefield,
        to: Some(ZoneKind::Graveyard),
    })
}

fn verb(r: &mut Reader<'_>, what: &Selector) -> Option<TriggerPattern> {
    let (from, to) = if r.word("enters") {
        // The older wording, "enters the battlefield", says the same.
        r.words("the battlefield");
        (None, Some(ZoneKind::Battlefield))
    } else if r.word("dies") || r.words("is put into a graveyard from the battlefield") {
        (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard))
    } else if let Some(elsewhere) = r.attempt(|r| from_elsewhere(r, what)) {
        return Some(elsewhere);
    } else if r.words("is put into a graveyard from anywhere") {
        (None, Some(ZoneKind::Graveyard))
    } else if r.words("leaves the battlefield") {
        (Some(ZoneKind::Battlefield), None)
    } else if r.word("attacks") {
        if r.words("enchanted player") {
            return Some(TriggerPattern::AttacksPlayer {
                what: what.clone(),
                at: Ref::AttachedPlayer,
            });
        }
        if r.words("the monarch") {
            return Some(TriggerPattern::AttacksPlayer {
                what: what.clone(),
                at: Ref::Monarch,
            });
        }
        if r.words("one of your opponents") {
            return Some(TriggerPattern::AttacksPlayer {
                what: what.clone(),
                at: Ref::Opponents,
            });
        }
        let attacks = TriggerPattern::Attacks { what: what.clone() };
        return Some(first_time(r, attacks));
    } else if r.words("becomes the target of a spell or ability") {
        return becomes_target(r, what);
    } else if r.words("is dealt damage") {
        let to = match what.0.as_slice() {
            [Pred::This] => Ref::This,
            [Pred::Is(Ref::Attached)] => Ref::Attached,
            _ => Ref::Each(what.clone()),
        };
        let damaged = TriggerPattern::DealsDamage {
            source: Selector::default(),
            to: Some(to),
            combat: None,
        };
        return Some(first_time(r, damaged));
    } else if r.word("blocks") {
        return Some(TriggerPattern::Blocks { what: what.clone() });
    } else if r.words("becomes blocked") {
        return Some(TriggerPattern::BecomesBlocked { what: what.clone() });
    } else if r.words("becomes tapped") {
        return Some(TriggerPattern::Tapped { what: what.clone() });
    } else if r.words("becomes untapped") {
        return Some(TriggerPattern::Untapped { what: what.clone() });
    } else if r.word("explores") {
        return Some(TriggerPattern::Explored { what: what.clone() });
    } else if r.words("becomes crewed") {
        return Some(first_time(
            r,
            TriggerPattern::BecomesCrewed { what: what.clone() },
        ));
    } else if r.words("becomes saddled") {
        return Some(first_time(
            r,
            TriggerPattern::BecomesSaddled { what: what.clone() },
        ));
    } else if r.words("becomes monstrous") {
        return Some(TriggerPattern::BecomesMonstrous { what: what.clone() });
    } else if r.words("becomes level") {
        return Some(TriggerPattern::BecomesLevel {
            what: what.clone(),
            level: r.number()?,
        });
    } else if r.words("is turned face up") {
        return Some(TriggerPattern::TurnedFaceUp { what: what.clone() });
    } else if r.word("mutates") {
        return Some(TriggerPattern::Mutates { what: what.clone() });
    } else if r.word("adapts") {
        return Some(TriggerPattern::Adapted { what: what.clone() });
    } else if r.word("connives") {
        return Some(TriggerPattern::Connived { what: what.clone() });
    } else if r.words("becomes attached to") {
        return Some(TriggerPattern::Attached {
            what: what.clone(),
            to: subject(r)?,
        });
    } else if r.word("deals") {
        return damage_dealt(r, what);
    } else {
        return None;
    };
    Some(TriggerPattern::ZoneChange {
        what: what.clone(),
        from,
        to,
    })
}

/// What follows "becomes the target of a spell or ability": "you control", "an opponent controls" or nothing, and
/// "for the first time each turn" (a count of the controller's own spells and abilities, so not for an opponent's).
fn becomes_target(r: &mut Reader<'_>, what: &Selector) -> Option<TriggerPattern> {
    let controller = if r.words("you control") {
        Some(Ref::You)
    } else if r.words("an opponent controls") {
        Some(Ref::Opponents)
    } else {
        None
    };
    let by = Selector(controller.iter().cloned().map(Pred::ControlledBy).collect());
    let target = TriggerPattern::BecomesTarget {
        what: what.clone(),
        by,
    };
    let counted = first_time(r, target);
    let opponents = controller == Some(Ref::Opponents);
    (!(opponents && matches!(counted, TriggerPattern::Nth { .. }))).then_some(counted)
}

/// The event as it stands, or its nth time of the turn when "for the first time each turn" follows: "for the second time
/// each turn".
fn first_time(r: &mut Reader<'_>, event: TriggerPattern) -> TriggerPattern {
    let nth = r.attempt(|r| {
        r.need("for the")?;
        let at = ORDINALS.iter().position(|word| r.word(word))?;
        r.need("time each turn")?;
        u32::try_from(at + 1).ok()
    });
    match nth {
        Some(nth) => TriggerPattern::Nth {
            event: Box::new(event),
            nth,
        },
        None => event,
    }
}

/// What follows "deals": "combat damage to a player", "damage to an opponent", "noncombat damage".
fn damage_dealt(r: &mut Reader<'_>, source: &Selector) -> Option<TriggerPattern> {
    let combat = if r.words("combat damage") {
        Some(true)
    } else if r.words("noncombat damage") {
        Some(false)
    } else {
        r.need("damage")?;
        None
    };
    let to = if r.words("to a player") {
        Some(Ref::EachPlayer)
    } else if r.words("to an opponent") {
        Some(Ref::Opponents)
    } else if r.words("to you") {
        Some(Ref::You)
    } else {
        None
    };
    Some(TriggerPattern::DealsDamage {
        source: source.clone(),
        to,
        combat,
    })
}

/// The condition of an intervening "if": "you have 40 or more life", "you're the monarch", "there is no monarch".
pub fn condition(r: &mut Reader<'_>) -> Option<Condition> {
    if let Some(paid) = r.attempt(|r| cost_paid(r, false)) {
        return Some(paid);
    }
    if let Some(spent) = r.attempt(|r| super::spent::was_spent(r, false)) {
        return Some(spent);
    }
    if let Some(monarch) = monarch_condition(r) {
        return Some(monarch);
    }
    if let Some(bare) = r.attempt(no_counters) {
        return Some(bare);
    }
    if let Some(fact) = super::facts::read(r) {
        return Some(fact);
    }
    // "if you control your commander", "if you control three or more artifacts"
    if let Some(control) = r.attempt(super::statics::control) {
        return Some(control);
    }
    r.need("you have")?;
    let number = i32::try_from(r.number()?).ok()?;
    let op = if r.words("or more") {
        Cmp::Ge
    } else if r.words("or less") {
        Cmp::Le
    } else {
        return None;
    };
    r.word("life").then_some(())?;
    Some(Condition::Compare {
        left: Amount::Life(Ref::You),
        op,
        right: Amount::Fixed(number),
    })
}

/// The alternative costs a permanent or spell can be asked about: "its prowl cost was paid".
const PAID_WAYS: [(&str, CastWay); 4] = [
    ("prowl", CastWay::Prowl),
    ("mayhem", CastWay::Mayhem),
    ("spectacle", CastWay::Spectacle),
    ("madness", CastWay::Madness),
];

/// "its prowl cost was paid", or "this spell's prowl cost was paid" with `spell`: it was cast for that alternative cost.
pub fn cost_paid(r: &mut Reader<'_>, spell: bool) -> Option<Condition> {
    // Gift (CR 702.174): the one optional additional cost a card with the keyword has.
    if r.words("the gift was promised") {
        return Some(Condition::AdditionalCostPaid);
    }
    // A card with two kickers names the one that was paid: "it was kicked with its {W} kicker".
    if !spell && r.words("it was kicked with its") {
        let cost = super::costs::cost(r)?;
        r.need("kicker")?;
        return Some(Condition::KickedWith(cost));
    }
    if spell {
        r.source().then_some(())?;
        if r.words("was kicked") {
            return Some(Condition::Kicked);
        }
        r.punct('\'').then_some(())?;
        r.need("s")?;
    } else {
        r.need("its")?;
    }
    let word = r.take_word()?;
    let way = PAID_WAYS.iter().find(|(name, _)| *name == word)?.1;
    r.need("cost was paid")?;
    Some(Condition::CastWith(way))
}

/// The condition `cost_paid` reads, written back for the same `spell`.
pub fn cost_paid_text(condition: &Condition, spell: bool) -> Option<String> {
    if *condition == Condition::AdditionalCostPaid {
        return Some("the gift was promised".to_owned());
    }
    if let Condition::KickedWith(cost) = condition {
        let cost = super::costs::text(cost)?;
        return (!spell).then(|| format!("it was kicked with its {cost} kicker"));
    }
    if *condition == Condition::Kicked {
        return spell.then(|| "this spell was kicked".to_owned());
    }
    let Condition::CastWith(way) = condition else {
        return None;
    };
    let name = PAID_WAYS.iter().find(|(_, paid)| paid == way)?.0;
    let owner = if spell { "this spell's" } else { "its" };
    Some(format!("{owner} {name} cost was paid"))
}

/// "while saddled" after an attack: the creature is saddled (CR 702.171b).
pub fn saddled() -> Condition {
    Condition::Matches {
        subject: Ref::This,
        selector: Selector(vec![Pred::HasKeyword(Keyword::Saddled)]),
    }
}

/// "~ has no charge counters on it": the card holds none of that counter, and "it" is the card in the effects that follow.
fn no_counters(r: &mut Reader<'_>) -> Option<Condition> {
    r.source().then_some(())?;
    r.need("has no")?;
    let counter = counters::kind(r)?;
    r.need("counters on")?;
    r.word("it").then_some(())?;
    r.it = Some(Ref::This);
    Some(Condition::Compare {
        left: Amount::Counters {
            on: Ref::This,
            counter,
        },
        op: Cmp::Eq,
        right: Amount::Fixed(0),
    })
}

/// Who is the monarch: "you're the monarch", "an opponent is the monarch", "there is no monarch".
pub fn monarch_condition(r: &mut Reader<'_>) -> Option<Condition> {
    if r.words("you're the monarch") {
        Some(Condition::Monarch(Ref::You))
    } else if r.words("an opponent is the monarch") {
        Some(Condition::Monarch(Ref::Opponents))
    } else if r.attacked_player && r.words("that player is the monarch") {
        Some(Condition::Monarch(Ref::Defending))
    } else if r.words("you were the monarch as the turn began") {
        Some(Condition::MonarchAtTurnStart(Ref::You))
    } else if r.words("an opponent was the monarch as the turn began") {
        Some(Condition::MonarchAtTurnStart(Ref::Opponents))
    } else if r.words("there is no monarch") {
        Some(no_monarch())
    } else if r.words("you have the city's blessing") {
        Some(Condition::CitysBlessing(Ref::You))
    } else {
        super::day_night::condition(r)
    }
}

/// "There is no monarch": no player is the monarch.
fn no_monarch() -> Condition {
    Condition::Not(Box::new(Condition::Monarch(Ref::EachPlayer)))
}

/// The condition `monarch_condition` reads, written back.
pub fn monarch_text(condition: &Condition) -> Option<String> {
    Some(match condition {
        Condition::Monarch(Ref::You) => "you're the monarch".to_owned(),
        Condition::Monarch(Ref::Opponents) => "an opponent is the monarch".to_owned(),
        Condition::Monarch(Ref::Defending) => "that player is the monarch".to_owned(),
        Condition::MonarchAtTurnStart(Ref::You) => {
            "you were the monarch as the turn began".to_owned()
        }
        Condition::MonarchAtTurnStart(Ref::Opponents) => {
            "an opponent was the monarch as the turn began".to_owned()
        }
        Condition::CitysBlessing(Ref::You) => "you have the city's blessing".to_owned(),
        condition if *condition == no_monarch() => "there is no monarch".to_owned(),
        _ => return super::day_night::condition_text(condition),
    })
}

/// The condition as the card words it, after "if".
pub fn condition_text(condition: &Condition) -> Option<String> {
    if let Some(paid) = cost_paid_text(condition, false) {
        return Some(paid);
    }
    if let Some(spent) = super::spent::was_spent_text(condition, false) {
        return Some(spent);
    }
    if let Some(monarch) = monarch_text(condition) {
        return Some(monarch);
    }
    if let Some(fact) = super::facts::texts(condition).into_iter().next() {
        return Some(fact);
    }
    if let Some(control) = super::statics::control_texts(condition).into_iter().next() {
        return Some(control);
    }
    if let Some(none) = super::as_long_as::none_texts(condition).into_iter().next() {
        return Some(none);
    }
    if let Condition::Compare {
        left: Amount::Counters {
            on: Ref::This,
            counter,
        },
        op: Cmp::Eq,
        right: Amount::Fixed(0),
    } = condition
    {
        return Some(format!(
            "~ has no {} counters on it",
            counters::kind_text(counter)?
        ));
    }
    let Condition::Compare {
        left: Amount::Life(Ref::You),
        op,
        right: Amount::Fixed(number),
    } = condition
    else {
        return None;
    };
    let more = match op {
        Cmp::Ge => "more",
        Cmp::Le => "less",
        _ => return None,
    };
    Some(format!("you have {number} or {more} life"))
}

/// "a" or "an" for the words after it ("an Elf", "an 8/8 token", "a 1/1 token").
pub fn article(words: &str) -> &'static str {
    let vowel = words
        .chars()
        .next()
        .is_some_and(|first| "aeiou".contains(first.to_ascii_lowercase()));
    let eight = ["8", "11", "18"].iter().any(|number| {
        words
            .strip_prefix(number)
            .is_some_and(|rest| !rest.starts_with(|c: char| c.is_ascii_digit()))
    });
    // "an X/X creature".
    let sized = words.starts_with("X/");
    if vowel || eight || sized { "an" } else { "a" }
}

/// A subject with its article: "~", "a creature you control", "another Elf".
pub(super) fn subject_text(what: &Selector) -> Option<String> {
    if what.0 == [Pred::This] {
        return Some("~".to_owned());
    }
    if what.0 == [Pred::Is(Ref::Attached)] {
        return Some("equipped creature".to_owned());
    }
    let text = phrase_text(what, Form::Subject)?;
    if what.0.contains(&Pred::Other) || own_commander(what) {
        Some(text)
    } else {
        Some(format!("{} {text}", article(&text)))
    }
}

/// Whether the selector is "your commander": a commander its owner designated.
fn own_commander(what: &Selector) -> bool {
    what.0.contains(&Pred::Commander) && what.0.contains(&Pred::OwnedBy(Ref::You))
}

/// "you cast", "an opponent casts", "a player casts" for the verb ("cast" or "sacrifice").
fn player_verb(by: &Ref, verb: &str) -> Option<String> {
    Some(match by {
        Ref::You => format!("you {verb}"),
        Ref::Opponents => format!("an opponent {verb}s"),
        Ref::EachPlayer => format!("a player {verb}s"),
        Ref::AttachedPlayer => format!("enchanted player {verb}s"),
        _ => return None,
    })
}

/// The subject of a pattern that is about one thing happening to it, and what happens.
fn happening(pattern: &TriggerPattern) -> Option<(Selector, String)> {
    let said = |what: &Selector, verb: &str| Some((what.clone(), verb.to_owned()));
    match pattern {
        TriggerPattern::ZoneChange { what, from, to } => {
            let verb = match (from, to) {
                (None, Some(ZoneKind::Battlefield)) => "enters",
                (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard)) => "dies",
                (None, Some(ZoneKind::Graveyard)) => "is put into a graveyard from anywhere",
                (Some(ZoneKind::Battlefield), None) => "leaves the battlefield",
                _ => return None,
            };
            said(what, verb)
        }
        TriggerPattern::ZoneChangeExcept {
            what,
            except: ZoneKind::Battlefield,
            to: Some(ZoneKind::Graveyard),
        } => {
            let mut rest = what.clone();
            let own = rest
                .0
                .iter()
                .position(|pred| *pred == Pred::OwnedBy(Ref::You));
            let whose = if let Some(at) = own {
                rest.0.remove(at);
                "your"
            } else {
                "a"
            };
            said(
                &rest,
                &format!("is put into {whose} graveyard from anywhere other than the battlefield"),
            )
        }
        TriggerPattern::Attacks { what } => said(what, "attacks"),
        TriggerPattern::AttacksPlayer {
            what,
            at: Ref::AttachedPlayer,
        } => said(what, "attacks enchanted player"),
        TriggerPattern::AttacksPlayer {
            what,
            at: Ref::Monarch,
        } => said(what, "attacks the monarch"),
        TriggerPattern::AttacksPlayer {
            what,
            at: Ref::Opponents,
        } => said(what, "attacks one of your opponents"),
        TriggerPattern::Blocks { what } => said(what, "blocks"),
        TriggerPattern::BecomesBlocked { what } => said(what, "becomes blocked"),
        TriggerPattern::Tapped { what } => said(what, "becomes tapped"),
        TriggerPattern::Untapped { what } => said(what, "becomes untapped"),
        TriggerPattern::Explored { what } => said(what, "explores"),
        TriggerPattern::Connived { what } => said(what, "connives"),
        TriggerPattern::BecomesMonstrous { what } => said(what, "becomes monstrous"),
        TriggerPattern::BecomesCrewed { what } => said(what, "becomes crewed"),
        TriggerPattern::BecomesSaddled { what } => said(what, "becomes saddled"),
        TriggerPattern::BecomesLevel { what, level } => {
            said(what, &format!("becomes level {level}"))
        }
        TriggerPattern::TurnedFaceUp { what } => said(what, "is turned face up"),
        TriggerPattern::Mutates { what } => said(what, "mutates"),
        TriggerPattern::Adapted { what } => said(what, "adapts"),
        TriggerPattern::Attached { what, to } if !what.is_empty() && !to.is_empty() => {
            said(what, &format!("becomes attached to {}", subject_text(to)?))
        }
        TriggerPattern::BecomesTarget { what, by } => {
            let whose = match by.0.as_slice() {
                [] => "",
                [Pred::ControlledBy(Ref::You)] => " you control",
                [Pred::ControlledBy(Ref::Opponents)] => " an opponent controls",
                _ => return None,
            };
            said(
                what,
                &format!("becomes the target of a spell or ability{whose}"),
            )
        }
        TriggerPattern::DealsDamage {
            source,
            to: Some(to),
            combat: None,
        } if source.is_empty() && !matches!(to, Ref::You | Ref::Opponents | Ref::EachPlayer) => {
            said(&damaged_subject(to)?, "is dealt damage")
        }
        TriggerPattern::DealsDamage { source, to, combat } => said(
            source,
            &format!("deals {}", damage_text(*combat, to.as_ref())?),
        ),
        _ => None,
    }
}

/// "combat damage to a player", "damage to an opponent", "noncombat damage".
fn damage_text(combat: Option<bool>, to: Option<&Ref>) -> Option<String> {
    let kind = match combat {
        Some(true) => "combat damage",
        Some(false) => "noncombat damage",
        None => "damage",
    };
    let to = match to {
        None => "",
        Some(Ref::EachPlayer) => " to a player",
        Some(Ref::Opponents) => " to an opponent",
        Some(Ref::You) => " to you",
        Some(_) => return None,
    };
    Some(format!("{kind}{to}"))
}

/// The step as the card words it after "the beginning of".
fn step_text(step: Step, whose: &Ref) -> Option<String> {
    if step == Step::BeginCombat {
        return match whose {
            Ref::You => Some("combat on your turn".to_owned()),
            Ref::EachPlayer => Some("each combat".to_owned()),
            _ => None,
        };
    }
    let (_, words) = STEPS.iter().find(|(named, _)| *named == step)?;
    let owner = match whose {
        Ref::You => "your",
        Ref::Opponents => "each opponent's",
        Ref::EachPlayer => "each",
        Ref::Monarch => "the monarch's",
        _ => return None,
    };
    Some(format!("{owner} {words}"))
}

/// A permanent tapped for mana: "Whenever you tap a land for mana", "Whenever enchanted land is tapped for mana".
fn tapped_for_mana_text(what: &Selector, by: &Ref) -> Option<String> {
    if *by == Ref::EachPlayer
        && what.0.len() > 1
        && what.0.first() == Some(&Pred::Is(Ref::Attached))
    {
        let land = phrase_text(&Selector(what.0[1..].to_vec()), Form::Subject)?;
        return Some(format!("Whenever enchanted {land} is tapped for mana"));
    }
    Some(format!(
        "Whenever {} {} for mana",
        player_verb(by, "tap")?,
        subject_text(what)?
    ))
}

/// The event as the card words it, from "When" to the end of the event.
pub fn pattern_text(pattern: &TriggerPattern) -> Option<String> {
    match pattern {
        TriggerPattern::StepBegins { step, whose } => {
            Some(format!("At the beginning of {}", step_text(*step, whose)?))
        }
        TriggerPattern::SpellCast { what, by: Ref::You } if what.0 == [Pred::This] => {
            Some("When you cast ~".to_owned())
        }
        TriggerPattern::SpellCast { what, by } => {
            let mut preds = vec![Pred::Spell];
            preds.extend(what.0.iter().cloned());
            let spell = subject_text(&Selector(preds))?;
            Some(format!("Whenever {} {spell}", player_verb(by, "cast")?))
        }
        TriggerPattern::Sacrificed { what, by } => Some(format!(
            "Whenever {} {}",
            player_verb(by, "sacrifice")?,
            subject_text(what)?
        )),
        TriggerPattern::BecomesMonarch { who } => Some(format!(
            "Whenever {} the monarch",
            player_verb(who, "become")?
        )),
        TriggerPattern::DayNightChanges { to } => Some(super::day_night::changes_text(*to)),
        TriggerPattern::Proliferated { who } => {
            Some(format!("Whenever {}", player_verb(who, "proliferate")?))
        }
        TriggerPattern::Amassed { who } => Some(format!("Whenever {}", player_verb(who, "amass")?)),
        TriggerPattern::Bolstered { who } => {
            Some(format!("Whenever {}", player_verb(who, "bolster")?))
        }
        TriggerPattern::Surveilled { who } => {
            Some(format!("Whenever {}", player_verb(who, "surveil")?))
        }
        TriggerPattern::LifeGained { who } => {
            Some(format!("Whenever {} life", player_verb(who, "gain")?))
        }
        TriggerPattern::LifeLost { who } => {
            Some(format!("Whenever {} life", player_verb(who, "lose")?))
        }
        TriggerPattern::Drew { who, what } if what.is_empty() => {
            Some(format!("Whenever {} a card", player_verb(who, "draw")?))
        }
        TriggerPattern::Discarded { what, by } if what.is_empty() => {
            Some(format!("Whenever {} a card", player_verb(by, "discard")?))
        }
        TriggerPattern::Cycled { what, by } => cycled_text(what, by),
        TriggerPattern::PlayerAttacked { who } => {
            let (subject, be) = match who {
                Ref::You => ("you", "are"),
                Ref::Opponents => ("an opponent", "is"),
                Ref::EachPlayer => ("a player", "is"),
                Ref::AttachedPlayer => ("enchanted player", "is"),
                _ => return None,
            };
            Some(format!("Whenever {subject} {be} attacked"))
        }
        TriggerPattern::PlayerAttacks { by, at } => {
            let at = match at {
                Ref::You => "you",
                Ref::AttachedPlayer => "enchanted player",
                _ => return None,
            };
            Some(format!(
                "Whenever {} {at} with one or more creatures",
                player_verb(by, "attack")?
            ))
        }
        TriggerPattern::TappedForMana { what, by } => tapped_for_mana_text(what, by),
        TriggerPattern::CountersPut { what, counter } => {
            counter_text(what, counter.as_ref(), "put on")
        }
        TriggerPattern::CountersRemoved { what, counter } => {
            counter_text(what, counter.as_ref(), "removed from")
        }
        TriggerPattern::Nth { event, nth } => nth_text(event, *nth),
        TriggerPattern::Batch(inner) => batch_text(inner),
        TriggerPattern::GroupAttacks { what, at_least } => {
            let count = if *at_least == 1 {
                "one"
            } else {
                count_word(*at_least)?
            };
            Some(format!(
                "Whenever {count} or more {} attack",
                phrase_text(what, Form::All)?
            ))
        }
        TriggerPattern::AnyOf(patterns) => any_of_text(patterns),
        _ => {
            let (what, verb) = happening(pattern)?;
            about(&what, &[verb])
        }
    }
}

/// "Whenever you cycle a card", "Whenever a player cycles another card" and "When you cycle ~".
fn cycled_text(what: &Selector, by: &Ref) -> Option<String> {
    let card = match what.0.as_slice() {
        [] => "a card",
        [Pred::Other] => "another card",
        [Pred::This] if *by == Ref::You => return Some("When you cycle ~".to_owned()),
        _ => return None,
    };
    Some(format!("Whenever {} {card}", player_verb(by, "cycle")?))
}

/// Several events of one subject ("~ enters or attacks"), or of the card and a second subject in turn: "Whenever ~ or
/// another Elf you control enters or dies".
fn any_of_text(patterns: &[TriggerPattern]) -> Option<String> {
    let happenings: Vec<(Selector, String)> =
        patterns.iter().map(happening).collect::<Option<Vec<_>>>()?;
    let (what, _) = happenings.first()?;
    if happenings.iter().all(|(other, _)| other == what) {
        let verbs: Vec<String> = happenings.iter().map(|(_, verb)| verb.clone()).collect();
        return about(what, &verbs);
    }
    let second = &happenings.get(1)?.0;
    let mut verbs = Vec::new();
    for pair in happenings.chunks(2) {
        let [(itself, verb), (other, same)] = pair else {
            return None;
        };
        if *itself != Selector::this() || other != second || verb != same {
            return None;
        }
        verbs.push(verb.as_str());
    }
    (second.0.contains(&Pred::Other) || second.0 == [Pred::Is(Ref::Attached)]).then_some(())?;
    Some(format!(
        "Whenever ~ or {} {}",
        subject_text(second)?,
        verbs.join(" or ")
    ))
}

/// What is dealt damage, as the subject of "is dealt damage": the card itself, what an Equipment or Aura is attached to
/// or the objects a selector names.
fn damaged_subject(to: &Ref) -> Option<Selector> {
    match to {
        Ref::This => Some(Selector::this()),
        Ref::Attached => Some(Selector(vec![Pred::Is(Ref::Attached)])),
        Ref::Each(what) => Some(what.clone()),
        _ => None,
    }
}

/// "Whenever you draw your second card each turn", "Whenever you cast your first spell each turn", "Whenever ~ attacks
/// for the first time each turn".
fn nth_text(event: &TriggerPattern, nth: u32) -> Option<String> {
    let ordinal = ORDINALS.get(usize::try_from(nth.checked_sub(1)?).ok()?)?;
    let (by, verb, noun) = match event {
        TriggerPattern::Drew { who, what } if what.is_empty() => (who, "draw", "card"),
        TriggerPattern::SpellCast { what, by } if what.is_empty() => (by, "cast", "spell"),
        // "Whenever you cast your fourth noncreature spell each turn".
        TriggerPattern::SpellCast { what, by: Ref::You } => {
            let mut preds = vec![Pred::Spell];
            preds.extend(what.0.iter().cloned());
            let spell = subject_text(&Selector(preds))?;
            let spell = spell.strip_prefix("a ").or(spell.strip_prefix("an "))?;
            return Some(format!(
                "Whenever you cast your {ordinal} {spell} each turn"
            ));
        }
        TriggerPattern::Attacks { .. }
        | TriggerPattern::BecomesCrewed { .. }
        | TriggerPattern::BecomesSaddled { .. }
        | TriggerPattern::BecomesTarget { .. }
        | TriggerPattern::DealsDamage { .. }
        | TriggerPattern::LifeGained { .. }
        | TriggerPattern::LifeLost { .. }
        | TriggerPattern::Cycled { .. }
        | TriggerPattern::Surveilled { .. } => {
            let counted = pattern_text(event)?;
            return Some(format!("{counted} for the {ordinal} time each turn"));
        }
        // The counters of one batch are one time: the first counter put in the turn is the one that counts.
        TriggerPattern::CountersPut { counter: None, .. } => {
            let counted = batch_text(event)?;
            return Some(format!("{counted} for the {ordinal} time each turn"));
        }
        TriggerPattern::ZoneChange {
            what,
            from: None,
            to: None | Some(ZoneKind::Battlefield),
        } if what.0 == [Pred::Token, Pred::ControlledBy(Ref::You)] && nth == 1 => {
            return Some(
                "Whenever you create one or more tokens for the first time each turn".to_owned(),
            );
        }
        _ => return None,
    };
    let whose = if *by == Ref::You { "your" } else { "their" };
    Some(format!(
        "Whenever {} {whose} {ordinal} {noun} each turn",
        player_verb(by, verb)?
    ))
}

/// "Whenever one or more creatures you control deal combat damage to a player", "Whenever one or more cards are put
/// into your graveyard from your library".
fn batch_text(inner: &TriggerPattern) -> Option<String> {
    match inner {
        TriggerPattern::DealsDamage { source, to, combat } => Some(format!(
            "Whenever one or more {} deal {}",
            phrase_text(source, Form::All)?,
            damage_text(*combat, to.as_ref())?
        )),
        TriggerPattern::ZoneChange {
            what,
            from: Some(ZoneKind::Library),
            to: Some(ZoneKind::Graveyard),
        } if what.0 == [Pred::OwnedBy(Ref::You)] => Some(
            "Whenever one or more cards are put into your graveyard from your library".to_owned(),
        ),
        TriggerPattern::ZoneChange { what, from, to } => batches::zone_change(what, *from, *to),
        TriggerPattern::CountersPut { what, counter } => {
            let kind = match counter {
                Some(counter) => format!("{} ", counters::kind_text(counter)?),
                None => String::new(),
            };
            Some(format!(
                "Whenever one or more {kind}counters are put on {}",
                subject_text(what)?
            ))
        }
        _ => None,
    }
}

/// "Whenever a +1/+1 counter is put on ~", "Whenever a counter is removed from a creature you control".
fn counter_text(what: &Selector, counter: Option<&CounterKind>, moved: &str) -> Option<String> {
    let place = if what.is_empty() {
        "a permanent".to_owned()
    } else {
        subject_text(what)?
    };
    Some(format!(
        "Whenever {} is {moved} {place}",
        counters::trigger_kind_text(counter)?
    ))
}

/// "When ~ enters", "Whenever a land you control enters", "Whenever ~ enters or attacks": "when" is
/// for the card itself entering, dying, leaving, becoming monstrous or being turned face up, "whenever" for the rest.
fn about(what: &Selector, verbs: &[String]) -> Option<String> {
    let once = what.0 == [Pred::This]
        && verbs.iter().all(|verb| {
            verb.starts_with("becomes level ")
                || matches!(
                    verb.as_str(),
                    "enters"
                        | "dies"
                        | "is put into a graveyard from anywhere"
                        | "leaves the battlefield"
                        | "becomes monstrous"
                        | "is turned face up"
                )
        });
    Some(format!(
        "{} {} {}",
        if once { "When" } else { "Whenever" },
        subject_text(what)?,
        verbs.join(" or ")
    ))
}
