//! r55-04 static abilities over cards in more zones than the battlefield, written as the cards word them: "All cards
//! that aren't on the battlefield, spells, and permanents are ...", "Nonland permanents you control are white. The same
//! is true for spells you control and nonland cards you own that aren't on the battlefield." and "Lands you control and
//! land cards in your library are basic."

use super::static_parts::predicates;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Condition;
use mtg_core::card::CardType;

/// Whether the static ability is a plain one of the battlefield: no condition, not defining.
const fn plain(condition: Option<&Condition>, defining: bool, from: ZoneKind) -> bool {
    condition.is_none() && !defining && matches!(from, ZoneKind::Battlefield)
}

/// The predicates of a group, each ending the sentence.
fn sentences(subject: &str, mods: &[Modification], tail: &str) -> Vec<String> {
    predicates(mods, true)
        .into_iter()
        .map(|predicate| format!("{subject} {predicate}.{tail}"))
        .collect()
}

/// "All cards that aren't on the battlefield, spells, and permanents are ...": every object of every zone.
pub(super) fn r5504_all_texts(
    mods: &[Modification],
    condition: Option<&Condition>,
    defining: bool,
    from: ZoneKind,
) -> Vec<String> {
    if !plain(condition, defining, from) {
        return Vec::new();
    }
    sentences(
        "All cards that aren't on the battlefield, spells, and permanents",
        mods,
        "",
    )
}

/// The rest of a selector over every zone that the card types of `ep22_everywhere` do not cover: "Nonland permanents
/// you control are white. The same is true for spells you control and nonland cards you own that aren't on the
/// battlefield." and "Slivers you control and nontoken creatures you control are the chosen type in addition to their
/// other creature types. The same is true for creature spells you control and creature cards you own that aren't on
/// the battlefield."; `rest` is the selector without its zones.
pub(super) fn r5504_rest_texts(
    rest: &[Pred],
    mods: &[Modification],
    condition: Option<&Condition>,
    defining: bool,
    from: ZoneKind,
) -> Vec<String> {
    if !plain(condition, defining, from) {
        return Vec::new();
    }
    let nonland = [
        Pred::Not(Box::new(Pred::Type(CardType::Land))),
        Pred::ControlledBy(Ref::You),
    ];
    if rest == nonland {
        let [Modification::SetColors(colors)] = mods else {
            return Vec::new();
        };
        let [color] = colors.as_slice() else {
            return Vec::new();
        };
        return vec![format!(
            "Nonland permanents you control are {}. The same is true for spells you control and nonland cards you own \
             that aren't on the battlefield.",
            format!("{color:?}").to_lowercase()
        )];
    }
    if rest == slivers_and_nontoken_creatures().as_slice()
        && mods == [Modification::AddChosenSubtype]
    {
        return vec![
            "Slivers you control and nontoken creatures you control are the chosen type in addition to their other \
             creature types. The same is true for creature spells you control and creature cards you own that aren't \
             on the battlefield."
                .to_owned(),
        ];
    }
    Vec::new()
}

/// "Slivers you control and nontoken creatures you control", and the creature spells and cards elsewhere: a Sliver or a
/// nontoken creature on the battlefield, a creature in any other zone (no card is a token there).
fn slivers_and_nontoken_creatures() -> Vec<Pred> {
    let sliver = || Pred::Subtype(mtg_core::card::Subtype("Sliver".to_owned()));
    let creature = || Pred::Type(CardType::Creature);
    vec![
        Pred::ControlledBy(Ref::You),
        Pred::AnyOf(vec![creature(), Pred::In(ZoneKind::Battlefield)]),
        Pred::AnyOf(vec![creature(), sliver()]),
        Pred::AnyOf(vec![sliver(), Pred::Not(Box::new(Pred::Token))]),
    ]
}

/// "Lands you control and land cards in your library are basic.": the lands of the battlefield and of the library of
/// the controller; empty for any other selector.
pub(super) fn r5504_texts(
    selector: &Selector,
    mods: &[Modification],
    condition: Option<&Condition>,
    defining: bool,
    from: ZoneKind,
) -> Vec<String> {
    if !plain(condition, defining, from) {
        return Vec::new();
    }
    let library = Pred::AnyOf(vec![
        Pred::In(ZoneKind::Battlefield),
        Pred::In(ZoneKind::Library),
    ]);
    if selector.0
        == [
            Pred::Type(CardType::Land),
            Pred::ControlledBy(Ref::You),
            library,
        ]
    {
        return sentences("Lands you control and land cards in your library", mods, "");
    }
    Vec::new()
}
