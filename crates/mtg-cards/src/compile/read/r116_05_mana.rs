//! Mana of one chosen color counted from the game: "Choose a color. Add an amount of mana of that color equal to the
//! number of opponents you attacked this turn.", "Choose a color. Add one mana of that color for each charge counter on
//! ~." Both are `Produce::AnyColor` of the count (one color for all of it), read and written back in either wording.

use super::amounts::{for_each, for_each_text};
use super::defining::value_text;
use super::spend;
use super::tokens::Reader;
use super::where_x;
use crate::ability::Ability;
use crate::effect::{Effect, Produce};
use crate::spend::ManaLasts;
use crate::value::Amount;

/// The words that open the second sentence of either wording.
const R116_EQUAL: &str = "Add an amount of mana of that color equal to";

/// "choose a color. Add an amount of mana of that color equal to ..." or "... Add one mana of that color for each ...",
/// with what the mana may be spent on after it.
pub(super) fn r116_chosen_color_mana(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("choose a color")?;
    r.punct('.').then_some(())?;
    r.need("add")?;
    let count = if r.words("an amount of mana of that color") {
        where_x::equal_to(r)?
    } else {
        (r.word("one") || r.number() == Some(1)).then_some(())?;
        r.need("mana of that color")?;
        for_each(r)?
    };
    if matches!(
        count,
        Amount::Fixed(_) | Amount::X | Amount::Triggering | Amount::Count(_)
    ) {
        return None;
    }
    let only = r
        .attempt(|r| {
            r.punct('.').then_some(())?;
            spend::only(r)
        })
        .unwrap_or_default();
    Some(vec![Effect::AddMana {
        who: crate::refs::Ref::You,
        mana: Produce::AnyColor(count),
        times: Amount::one(),
        only,
        lasts: ManaLasts::Step,
        when_spent: None,
    }])
}

/// The first sentence and the mana of the count: "choose a color. Add an amount of mana of that color equal to ...";
/// `None` for a count a number or X says, and for a count of objects, which "X mana of any one color, where X is the
/// number of ..." says.
pub(super) fn r116_chosen_color_text(count: &Amount) -> Option<String> {
    if matches!(
        count,
        Amount::Fixed(_) | Amount::X | Amount::Triggering | Amount::Count(_)
    ) {
        return None;
    }
    Some(format!(
        "choose a color. {R116_EQUAL} {}",
        value_text(count)?
    ))
}

/// The wordings "Choose a color. Add an amount of mana of that color equal to ..." and "... Add one mana of that color
/// for each ..." beside each "Add X mana of any one color, where X is ..." wording of the ability's count.
pub(super) fn r116_chosen_color_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let counts: Vec<Amount> = ability
        .effects()
        .iter()
        .filter_map(|effect| match effect {
            Effect::AddMana {
                mana: Produce::AnyColor(count),
                ..
            } => Some(count.clone()),
            _ => None,
        })
        .collect();
    let mut more = Vec::new();
    for count in counts {
        let Some(equal) = value_text(&count) else {
            continue;
        };
        let mut intos = vec![format!("Choose a color. {R116_EQUAL} {equal}")];
        intos.extend(
            for_each_text(&count)
                .map(|each| format!("Choose a color. Add one mana of that color for each {each}")),
        );
        for from in [
            format!("Add X mana of any one color, where X is {equal}"),
            format!("add X mana of any one color, where X is {equal}"),
        ] {
            for text in texts.iter().filter(|text| text.contains(&from)) {
                more.extend(intos.iter().map(|into| text.replacen(&from, into, 1)));
            }
        }
    }
    texts.extend(more);
}
