//! r47-03 readers of whole static abilities: a loss of life made a multiple ("If an opponent would lose life during your
//! turn, they lose twice that much life instead."). Each reading is kept only when its writer gives the text back.

use super::tokens::{Reader, same_text, tokenize};
use crate::ability::Ability;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Condition;

/// Reads a whole static ability of the r47-03 wordings, kept only when it is written back as `text`.
pub(super) fn r4703_static_read(text: &str) -> Option<Ability> {
    let found = r4703_loss_times(text).or_else(|| r4703_counts_as_named(text))?;
    crate::compile::written_variants(&found)
        .iter()
        .any(|written| same_text(written, text))
        .then_some(found)
}

/// The players a loss of life names and the pronoun of what they lose: "an opponent ... they", "a player ... they",
/// "you ... you".
pub(in crate::compile::read) const R4703_LOSERS: [(&str, Ref, &str); 3] = [
    ("an opponent", Ref::Opponents, "they"),
    ("a player", Ref::EachPlayer, "they"),
    ("you", Ref::You, "you"),
];

/// The factors of "<n> times that much life": the word and the factor.
pub(in crate::compile::read) const R4703_TIMES: [(&str, u32); 2] =
    [("twice", 2), ("three times", 3)];

/// The words of [`r4703_copy_all`].
const R4703_COPY_ALL: &str = "copy all spells you control, then copy all other activated and triggered abilities you control. You may choose new targets for the copies";

/// The two copies of [`R4703_COPY_ALL`].
fn r4703_copy_all_effects() -> Vec<crate::effect::Effect> {
    let mine = |kind: crate::refs::Pred| {
        Ref::Each(crate::refs::Selector(vec![
            kind,
            crate::refs::Pred::ControlledBy(Ref::You),
        ]))
    };
    vec![
        crate::effect::Effect::CopySpell {
            what: mine(crate::refs::Pred::Spell),
            who: Ref::You,
            count: crate::value::Amount::one(),
            new_targets: true,
            except: Vec::new(),
            each_target: None,
        },
        crate::effect::Effect::CopyAbility {
            what: mine(crate::refs::Pred::AnyOf(vec![
                crate::refs::Pred::Ability(crate::refs::AbilityKind::Activated),
                crate::refs::Pred::Ability(crate::refs::AbilityKind::Triggered),
            ])),
            count: crate::value::Amount::one(),
            new_targets: true,
        },
    ]
}

/// "copy all spells you control, then copy all other activated and triggered abilities you control. You may choose new
/// targets for the copies" (Ulalek, Fused Atrocity): the ability resolving is not copied.
pub(super) fn r4703_copy_all(r: &mut Reader<'_>) -> Option<Vec<crate::effect::Effect>> {
    r.words(R4703_COPY_ALL).then(r4703_copy_all_effects)
}

/// What [`r4703_copy_all`] reads, written, and the number of effects it takes.
pub(in crate::compile::read) fn r4703_copy_all_text(
    effects: &[crate::effect::Effect],
) -> Option<(String, usize)> {
    let wanted = r4703_copy_all_effects();
    effects
        .starts_with(&wanted)
        .then(|| (R4703_COPY_ALL.to_owned(), wanted.len()))
}

/// The words after "for each" of [`r4703_sacrificed_types`].
const R4703_SACRIFICED_TYPES: &str = "card type among permanents you've sacrificed this turn";

/// The card types among the permanents the controller sacrificed this turn.
fn r4703_sacrificed_types_count() -> crate::value::Amount {
    crate::value::Amount::TallyTypes {
        event: crate::value::TallyEvent::Sacrificed,
        who: Ref::You,
        filter: crate::refs::Selector::default(),
    }
}

/// "for each card type among permanents you've sacrificed this turn" (Korvold, Gleeful Glutton).
pub(super) fn r4703_sacrificed_types(r: &mut Reader<'_>) -> Option<crate::value::Amount> {
    r.need("for each")?;
    r.words(R4703_SACRIFICED_TYPES)
        .then(r4703_sacrificed_types_count)
}

/// The words after "for each" of the count [`r4703_sacrificed_types`] reads; `None` for any other.
pub(super) fn r4703_sacrificed_types_text(per: &crate::value::Amount) -> Option<String> {
    (*per == r4703_sacrificed_types_count()).then(|| R4703_SACRIFICED_TYPES.to_owned())
}

/// A card name of one or more capitalized words ("Muscle Burst").
fn r4703_name(r: &mut Reader<'_>) -> Option<String> {
    let mut words = vec![r.name()?.to_owned()];
    while let Some(word) = r.attempt(|r| r.name().map(str::to_owned)) {
        words.push(word);
    }
    Some(words.join(" "))
}

/// "If this card is in a graveyard, effects from spells named <name> count it as a card named <name>."
fn r4703_counts_as_named(text: &str) -> Option<Ability> {
    let tokens = tokenize(text);
    let mut r = Reader::new(&tokens, false);
    r.need("if")?;
    (r.source() || r.words("this card")).then_some(())?;
    r.need("is in a graveyard")?;
    r.punct(',').then_some(())?;
    r.need("effects from spells named")?;
    let name = r4703_name(&mut r)?;
    r.need("count it as a card named")?;
    (r4703_name(&mut r)? == name).then_some(())?;
    (r.sentence_end() && r.end()).then_some(())?;
    Some(Ability::Rule {
        rule: crate::modify::GameRule::CountsAsNamed(name),
        condition: None,
        from: ZoneKind::Graveyard,
    })
}

/// "If <player> would lose life[ during your turn], <they> lose <twice> that much life instead."
fn r4703_loss_times(text: &str) -> Option<Ability> {
    let tokens = tokenize(text);
    let mut r = Reader::new(&tokens, false);
    r.need("if")?;
    let (who, pronoun) = R4703_LOSERS
        .iter()
        .find(|(words, ..)| r.words(words))
        .map(|(_, who, pronoun)| (who.clone(), *pronoun))?;
    r.need("would lose life")?;
    let condition = r.words("during your turn").then_some(Condition::YourTurn);
    r.punct(',').then_some(())?;
    r.need(pronoun)?;
    r.need("lose")?;
    let times = R4703_TIMES
        .iter()
        .find(|(words, _)| r.words(words))
        .map(|(_, times)| *times)?;
    r.need("that much life instead")?;
    (r.sentence_end() && r.end()).then_some(())?;
    Some(Ability::Replacement {
        when: ReplaceWhen::LifeLoss { who },
        by: ReplaceBy::LossTimes(times),
        condition,
        from: ZoneKind::Battlefield,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a loss of life made a multiple reads as the replacement its writer gives back, and a near miss (a gain, the
    //! wrong pronoun, another turn, no "instead") reads nothing.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::opponent_your_turn(
        "If an opponent would lose life during your turn, they lose twice that much life instead.",
        true
    )]
    #[case::any_time(
        "If an opponent would lose life, they lose twice that much life instead.",
        true
    )]
    #[case::you("If you would lose life, you lose twice that much life instead.", true)]
    #[case::gain(
        "If an opponent would gain life during your turn, they gain twice that much life instead.",
        false
    )]
    #[case::pronoun(
        "If an opponent would lose life during your turn, you lose twice that much life instead.",
        false
    )]
    #[case::their_turn(
        "If an opponent would lose life during their turn, they lose twice that much life instead.",
        false
    )]
    #[case::farmhand(
        "If this card is in a graveyard, effects from spells named Muscle Burst count it as a card named Muscle Burst.",
        true
    )]
    #[case::another_name(
        "If this card is in a graveyard, effects from spells named Muscle Burst count it as a card named Flame Burst.",
        false
    )]
    #[case::in_exile(
        "If this card is in exile, effects from spells named Muscle Burst count it as a card named Muscle Burst.",
        false
    )]
    #[case::not_instead(
        "If an opponent would lose life during your turn, they lose twice that much life.",
        false
    )]
    fn r4703_loss_times_reads(#[case] text: &str, #[case] reads: bool) {
        assert_eq!(r4703_static_read(text).is_some(), reads, "{text}");
    }

    /// The compiler reaches the readers of the pack through its templates.
    #[rstest]
    #[case::rowan(
        "{T}: Spells you cast this turn that are black and/or red cost {X} less to cast, where X is the amount of life you lost this turn. Activate only as a sorcery.",
        crate::compile::ClauseKind::Activated
    )]
    #[case::farmhand(
        "If ~ is in a graveyard, effects from spells named Muscle Burst count it as a card named Muscle Burst.",
        crate::compile::ClauseKind::Static
    )]
    fn r4703_the_compiler_reads(#[case] text: &str, #[case] kind: crate::compile::ClauseKind) {
        let read = super::super::clause_with(text, kind, false, &super::super::Own::default());
        assert!(
            read.as_ref()
                .is_some_and(|ability| !matches!(ability, Ability::Manual { .. })),
            "{text}: {read:?}"
        );
    }
}
