//! What asks a decision: the spell or ability it belongs to, as the seat that answers may see it.
//!
//! The table shows the asking card beside the question, marks the asked clause in its rules text and places the
//! question in the run of triggers it is one of (`docs/protocol.md`, Decisions).

use std::sync::Arc;

use mtg_cards::CardIdentity;
use mtg_cards::ability::Ability;
use mtg_cards::compile::{AskedPlace, asked_clause, rules_words};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::resolve::{AskedAt, AskedPart, asked_at, stack_rules};
use mtg_engine::state::GameState;
use mtg_engine::triggers::{effect_words, public_cause};
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, DecisionKind};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::view::ObjectView;

/// The spell or ability a decision belongs to, for the seat that answers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub struct AskingView {
    /// The spell or ability on the stack that asks; left out when what asks is not on the stack (a cost being paid for
    /// an ability, a trigger whose targets are chosen as it goes on the stack).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stack: Option<ObjectId>,
    /// Whether that spell or ability is resolving now; false while it is being cast or put on the stack.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(as = "Option<bool>", optional)]
    pub resolving: bool,
    /// The card it comes from (the spell itself, the source of an ability) while the viewer sees that object; left out
    /// once the source has left its zone or is hidden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<ObjectId>,
    /// The name of the source as the viewer reads it, kept after the source has left; empty while it is hidden.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub name: String,
    /// The printed card of the source as last seen, for its picture once the source has left its zone; left out while
    /// `source` is set, for a source with no printed card (a token) and while the source is hidden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub identity: Option<CardIdentity>,
    /// The index of the asking ability among the rules of its source; left out for a spell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ability: Option<usize>,
    /// The rules text of the asking spell or ability in the card writer's words, its card's name for `~`; left out for
    /// rules the writer cannot word and while the source is hidden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
    /// The clause of `text` the decision asks about, as the first and one past the last UTF-16 code unit: the effect the
    /// resolution stands at or the modes chosen (`mtg_engine::resolve::asked_at`), else the "you may" or the target's
    /// words of the question; left out when no clause is found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub clause: Option<(u32, u32)>,
    /// What triggered the asking ability, while every player sees that object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cause: Option<AskingCause>,
    /// The run of triggered abilities the asking one went on the stack with, when there are two or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub run: Option<RunView>,
}

/// The object whose event triggered an ability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AskingCause {
    /// The object: the creature that entered, the spell that was cast.
    pub object: ObjectId,
    /// Its name as the viewer reads it.
    pub name: String,
    /// Whether the event is the object entering the battlefield.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(as = "Option<bool>", optional)]
    pub entered: bool,
}

/// Triggered abilities one player put on the stack together, in the order they resolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RunView {
    /// Tells this run from the others on the stack.
    pub id: u32,
    /// The place of the asking ability among `entries`, from 1.
    pub index: u32,
    /// Every ability of the run, the first to resolve first.
    pub entries: Vec<RunEntryView>,
}

/// One triggered ability of a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RunEntryView {
    /// Its object on the stack; left out once it has resolved or left the stack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub object: Option<ObjectId>,
    /// Its source while the viewer sees that object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<ObjectId>,
    /// The name of its source as the viewer reads it; empty while hidden.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub name: String,
    /// What it does in the words of its card, without the event it waits for; left out once it is gone, for an ability
    /// the writer cannot word and while the source is hidden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
    /// Whether it has resolved or left the stack.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(as = "Option<bool>", optional)]
    pub done: bool,
}

/// The object a decision names as what asks, and the question in the card's words where the decision carries one.
fn asked(kind: &DecisionKind) -> Option<(ObjectId, Question<'_>)> {
    Some(match kind {
        DecisionKind::YesNo(options) => (options.source, Question::May(&options.text)),
        DecisionKind::ChooseTargets(options) => {
            (options.source, Question::Phrase(&options.wording))
        }
        DecisionKind::PayMana(options) => (options.spell, Question::None),
        DecisionKind::ChooseMode(options) => (options.source, Question::None),
        DecisionKind::ChooseNumber(options) => (options.source, Question::None),
        DecisionKind::ChooseName(options) => (options.source, Question::None),
        DecisionKind::DivideAmount(options) => (options.source, Question::None),
        DecisionKind::ChooseCards(options) => (options.source, Question::None),
        DecisionKind::OrderCards(options) => (options.source, Question::None),
        DecisionKind::Scry(options) => (options.source, Question::None),
        DecisionKind::ChooseOption(options) => (options.source, Question::None),
        DecisionKind::ChooseCosts(options) => (options.source, Question::None),
        DecisionKind::R706ChooseDice(options) => (options.source?, Question::None),
        _ => return None,
    })
}

/// What a decision asks, for finding its clause in the rules text.
#[derive(Clone, Copy)]
enum Question<'a> {
    /// Nothing to look for.
    None,
    /// A "you may": the question in the card's words ("Pay {1}?").
    May(&'a str),
    /// Words of the text itself ("target creature you control").
    Phrase(&'a str),
}

impl AskingView {
    /// What asks `decision`, as `viewer` may see it; `None` for a decision no spell or ability asks (priority, combat,
    /// the opening hand).
    pub fn of(state: &GameState, viewer: PlayerId, decision: &Decision) -> Option<AskingView> {
        let (named, question) = asked(&decision.kind)?;
        let stack = asking_stack_object(state, named);
        let Some(stack) = stack else {
            // Not on the stack: the named object itself, while the viewer sees it.
            let view = ObjectView::of(state, viewer, named)?;
            // A card being cast off the battlefield reads as the spell it becomes.
            let (text, clause) = state
                .object(named)
                .filter(|object| object.ability().is_none() && object.zone() != Zone::Battlefield)
                .map(|_| spell_words(state, named, &decision.kind, &view.name, question))
                .unwrap_or_default();
            return Some(AskingView {
                source: Some(named),
                name: view.name,
                text,
                clause,
                ..AskingView::default()
            });
        };
        let object = state.object(stack)?;
        let view = ObjectView::of(state, viewer, stack)?;
        let resolving = state
            .resolving()
            .is_some_and(|running| running.object() == stack);
        let asked = asked_at(state, stack, &decision.kind);
        let Some(ability) = object.ability() else {
            let (text, clause) = spell_words(state, stack, &decision.kind, &view.name, question);
            return Some(AskingView {
                stack: Some(stack),
                resolving,
                source: Some(stack),
                name: view.name,
                text,
                clause,
                ..AskingView::default()
            });
        };
        let source = ability.source();
        let seen = state
            .object(source.id)
            .filter(|found| found.counter() == source.counter)
            .and_then(|_| ObjectView::of(state, viewer, source.id));
        // An ability keeps its source's card, so the card the source last was shows once the source has left.
        let identity = object
            .card()
            .identity
            .clone()
            .filter(|_| seen.is_none() && !view.name.is_empty());
        let text = view.rules_text;
        let marked = text
            .as_deref()
            .and_then(|text| marked(text, asked.as_ref(), &view.name, question));
        let cause = ability.cause().and_then(|cause| {
            let object = public_cause(state, cause)?;
            let name = ObjectView::of(state, viewer, object)?.name;
            Some(AskingCause {
                object,
                name,
                entered: text.as_deref().is_some_and(waits_for_entering),
            })
        });
        Some(AskingView {
            stack: Some(stack),
            resolving,
            source: seen.map(|seen| seen.id),
            name: view.name,
            identity,
            ability: Some(ability.index()),
            text,
            clause: marked,
            cause,
            run: run_of(state, viewer, stack),
        })
    }
}

/// The spell or ability on the stack that asks about `named`: the one resolving when it is `named` or comes from it,
/// else `named` itself while it is on the stack.
fn asking_stack_object(state: &GameState, named: ObjectId) -> Option<ObjectId> {
    let on_stack = |id: ObjectId| {
        state
            .object(id)
            .filter(|object| object.zone() == Zone::Stack)
    };
    if let Some(resolving) = state
        .resolving()
        .and_then(|running| on_stack(running.object()))
    {
        let from = resolving.ability().map(|ability| ability.source().id);
        if resolving.id() == named || from == Some(named) {
            return Some(resolving.id());
        }
    }
    on_stack(named).map(mtg_engine::object::GameObject::id)
}

/// Whether the event a triggered ability's text waits for is something entering the battlefield.
fn waits_for_entering(text: &str) -> bool {
    text.split(", ")
        .next()
        .is_some_and(|event| event.contains(" enters"))
}

/// The words of the spell `id` runs, its name `name` for `~`, and the clause `kind` asks about; nothing while the viewer
/// does not see its name or the writer cannot word it.
fn spell_words(
    state: &GameState,
    id: ObjectId,
    kind: &DecisionKind,
    name: &str,
    question: Question<'_>,
) -> (Option<String>, Option<(u32, u32)>) {
    let asked = asked_at(state, id, kind);
    let text = asked
        .as_ref()
        .map(|asked| Arc::clone(&asked.rules))
        .or_else(|| stack_rules(state, id))
        .filter(|_| !name.is_empty())
        .and_then(|rules| rules_words(&rules))
        .map(|words| words.replace('~', name));
    let clause = text
        .as_deref()
        .and_then(|text| marked(text, asked.as_ref(), name, question));
    (text, clause)
}

/// The clause of `text` (the words of the asking rules, `name` for `~`) the decision asks about, in UTF-16 code units:
/// the place the engine asks at, else the words of the question.
fn marked(
    text: &str,
    asked: Option<&AskedAt>,
    name: &str,
    question: Question<'_>,
) -> Option<(u32, u32)> {
    let placed = asked.and_then(|asked| {
        let place = match &asked.part {
            AskedPart::Modes(path) => AskedPlace::Modes(path),
            AskedPart::Effect(path) => AskedPlace::Effect(path),
        };
        asked_clause(&asked.rules, place, name, text)
    });
    match placed {
        Some(span) => utf16_span(text, span.start, span.end),
        None => clause_of(text, question),
    }
}

/// The byte span `start..end` of `text` in UTF-16 code units.
fn utf16_span(text: &str, start: usize, end: usize) -> Option<(u32, u32)> {
    let units = |to: usize| u32::try_from(text.get(..to)?.encode_utf16().count()).ok();
    Some((units(start)?, units(end)?))
}

/// The run `stack` is one of, when it holds two or more abilities.
fn run_of(state: &GameState, viewer: PlayerId, stack: ObjectId) -> Option<RunView> {
    let run = state.trigger_run_of(stack)?;
    let members: Vec<_> = run.resolving_order().collect();
    if members.len() < 2 {
        return None;
    }
    let index = members.iter().position(|member| member.object == stack)?;
    let entries = members
        .iter()
        .map(|member| {
            let waiting = state
                .object(member.object)
                .filter(|object| object.zone() == Zone::Stack);
            let source = state
                .object(member.source.id)
                .filter(|found| found.counter() == member.source.counter)
                .and_then(|_| ObjectView::of(state, viewer, member.source.id));
            match waiting
                .and_then(|object| Some((object, ObjectView::of(state, viewer, object.id())?)))
            {
                Some((object, view)) => {
                    let text = object
                        .ability()
                        .filter(|_| !view.name.is_empty())
                        .map(|ability| effect_of(ability.rules(), &view.name))
                        .filter(|text| !text.is_empty());
                    RunEntryView {
                        object: Some(member.object),
                        source: source.map(|source| source.id),
                        name: view.name,
                        text,
                        done: false,
                    }
                }
                None => RunEntryView {
                    object: None,
                    name: source
                        .as_ref()
                        .map(|source| source.name.clone())
                        .unwrap_or_default(),
                    source: source.map(|source| source.id),
                    text: None,
                    done: true,
                },
            }
        })
        .collect();
    Some(RunView {
        id: run.id(),
        index: u32::try_from(index + 1).unwrap_or(u32::MAX),
        entries,
    })
}

/// What `rules` does in the card's words, the source by `name`.
fn effect_of(rules: &Ability, name: &str) -> String {
    effect_words(rules, name)
}

/// The sentences that carry on a "you may": what happens if the player does or does not.
const FOLLOW_UPS: [&str; 6] = [
    "If you do,",
    "If you don't,",
    "If you can't,",
    "When you do,",
    "If they do,",
    "If they don't,",
];

/// The clause of `text` that `question` asks about, in UTF-16 code units.
fn clause_of(text: &str, question: Question<'_>) -> Option<(u32, u32)> {
    let lower = text.to_ascii_lowercase();
    let (start, end) = match question {
        Question::None => return None,
        Question::Phrase(words) => {
            let words = words.trim().to_ascii_lowercase();
            if words.is_empty() {
                return None;
            }
            let at = lower.find(&words)?;
            (at, at + words.len())
        }
        Question::May(asked) => {
            const MAY: &str = "you may ";
            let stem = asked.trim().trim_end_matches('?').to_ascii_lowercase();
            let found = (!stem.is_empty())
                .then(|| lower.find(&stem))
                .flatten()
                .map(|at| {
                    if lower[..at].ends_with(MAY) {
                        at - MAY.len()
                    } else {
                        at
                    }
                });
            let start = found.or_else(|| lower.find(MAY))?;
            let mut end = sentence_end(text, start);
            // "If you do, draw a card." belongs to the question it follows.
            while let Some(rest) = text.get(end..).map(str::trim_start)
                && FOLLOW_UPS.iter().any(|lead| rest.starts_with(lead))
            {
                end = sentence_end(text, text.len() - rest.len());
            }
            (start, end)
        }
    };
    utf16_span(text, start, end)
}

/// One past the period that ends the sentence `from` lies in; the end of `text` when it has none.
fn sentence_end(text: &str, from: usize) -> usize {
    text.get(from..)
        .and_then(|rest| rest.find(". ").map(|at| from + at + 1))
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    //! Goal: the asked clause of a rules text is the "you may" sentence the question names with the sentences that
    //! depend on it, or the words of a target, as UTF-16 offsets; a text without the clause marks nothing.
    use rstest::rstest;

    use super::*;

    fn marked(text: &str, question: Question<'_>) -> Option<String> {
        let (start, end) = clause_of(text, question)?;
        let units: Vec<u16> = text.encode_utf16().collect();
        String::from_utf16(&units[start as usize..end as usize]).ok()
    }

    #[rstest]
    #[case::pay_then_if_you_do(
        "Whenever another creature you control with power 2 or less enters, you may pay {1}. If you do, draw a card.",
        "Pay {1}?",
        Some("you may pay {1}. If you do, draw a card.")
    )]
    #[case::a_plain_may(
        "Whenever another creature enters, you may gain 1 life.",
        "Gain 1 life?",
        Some("you may gain 1 life.")
    )]
    #[case::the_second_may_of_two(
        "When Seer enters, you may draw a card. Then you may discard a card. If you do, gain 2 life.",
        "Discard a card?",
        Some("you may discard a card. If you do, gain 2 life.")
    )]
    #[case::a_question_in_other_words_marks_the_first_may(
        "When Seer enters, you may put a +1/+1 counter on it. Draw a card.",
        "Grow?",
        Some("you may put a +1/+1 counter on it.")
    )]
    #[case::no_question_marks_the_first_may(
        "At the beginning of your upkeep, you may scry 1.",
        "",
        Some("you may scry 1.")
    )]
    #[case::offsets_count_utf16_units(
        "Choose one — you may draw a card.",
        "Draw a card?",
        Some("you may draw a card.")
    )]
    #[case::nothing_to_mark("When Seer enters, draw a card.", "Draw a card?", Some("draw a card."))]
    #[case::no_clause("When Seer enters, each player loses 1 life.", "Pay {2}?", None)]
    fn a_may_marks_its_sentence_and_what_depends_on_it(
        #[case] text: &str,
        #[case] question: &str,
        #[case] clause: Option<&str>,
    ) {
        assert_eq!(marked(text, Question::May(question)).as_deref(), clause);
    }

    #[rstest]
    #[case::found(
        "When Seer enters, destroy target creature you don't control.",
        "target creature you don't control",
        Some("target creature you don't control")
    )]
    #[case::any_case("Target player draws a card.", "target player", Some("Target player"))]
    #[case::absent("Draw a card.", "target player", None)]
    #[case::empty("Draw a card.", "", None)]
    fn a_target_marks_its_words(
        #[case] text: &str,
        #[case] words: &str,
        #[case] clause: Option<&str>,
    ) {
        assert_eq!(marked(text, Question::Phrase(words)).as_deref(), clause);
    }

    #[test]
    fn a_decision_without_a_question_marks_nothing() {
        assert_eq!(clause_of("You may draw a card.", Question::None), None);
    }

    #[rstest]
    #[case::enters(
        "Whenever another creature you control enters, you may gain 1 life.",
        true
    )]
    #[case::dies("Whenever a creature dies, target player enters the dungeon.", false)]
    #[case::upkeep("At the beginning of your upkeep, draw a card.", false)]
    fn the_event_is_an_entering_only_before_the_first_comma(
        #[case] text: &str,
        #[case] entered: bool,
    ) {
        assert_eq!(waits_for_entering(text), entered);
    }
}
