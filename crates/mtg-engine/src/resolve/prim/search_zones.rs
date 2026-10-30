//! `Effect::Search` of several zones: "search your library, hand, and/or graveyard for a Rune card" (CR 701.23).

use mtg_cards::effect::Effect;
use mtg_cards::refs::ZoneKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::search::zone_of;
use super::wrong_effect;
use crate::choose::{ChoiceOption, Questions, Stop, Target};
use crate::events::Event;
use crate::library::{CardsFound, Revealed};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::rules::cant_search;
use crate::wording;

/// Every non-empty choice of the zones, the whole list first and then the smaller ones, each in the order the card
/// names its zones.
fn choices(zones: &[ZoneKind]) -> Vec<Vec<ZoneKind>> {
    let mut subsets: Vec<Vec<ZoneKind>> = (1u32..1 << zones.len())
        .map(|mask| {
            zones
                .iter()
                .enumerate()
                .filter(|(at, _)| mask >> at & 1 == 1)
                .map(|(_, zone)| *zone)
                .collect()
        })
        .collect();
    subsets.sort_by_key(|subset| std::cmp::Reverse(subset.len()));
    subsets
}

/// The words of a zone in a label.
fn label(zone: ZoneKind) -> &'static str {
    match zone {
        ZoneKind::Library => "library",
        ZoneKind::Hand => "hand",
        ZoneKind::Graveyard => "graveyard",
        ZoneKind::Exile => "exile",
        ZoneKind::Battlefield => "battlefield",
        ZoneKind::Stack => "stack",
        ZoneKind::Command => "command zone",
    }
}

/// The label of a choice of zones: "Library, hand and graveyard".
fn labelled(subset: &[ZoneKind]) -> String {
    let names: Vec<&str> = subset.iter().map(|zone| label(*zone)).collect();
    let text = match names.as_slice() {
        [init @ .., last] if !init.is_empty() => format!("{} and {last}", init.join(", ")),
        _ => names.join(""),
    };
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Each player the effect names who is still in the game, in turn order, chooses which of the zones to search (a
/// library that can't be searched is not among them), searches those zones for the cards that match the filter and
/// chooses between `count.min` and `count.max` of them among all of those zones; a library or a hand does not have to
/// give up what it holds (CR 701.23b), so the fewest is then 0. Each zone searched is one `CardsFound` for the
/// searcher, also when it held nothing that matches, and the cards found are bound to the name. A player who has just
/// one zone to search is not asked which. Nothing matching is no question.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Search {
        who,
        zone,
        also,
        filter,
        count,
        reveal,
        bind,
    } = effect
    else {
        return Err(wrong_effect("Search"));
    };
    let state = cx.state();
    let most = match &count.max {
        None => usize::MAX,
        Some(amount) => usize::try_from(cx.scope.amount(amount)?.max(0)).unwrap_or(usize::MAX),
    };
    let required = usize::try_from(cx.scope.amount(&count.min)?.max(0)).unwrap_or(usize::MAX);
    let mut asked = Questions::new(cx);
    let mut events = Vec::new();
    let mut found: Vec<Target> = Vec::new();
    for player in cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
    {
        let named: Vec<ZoneKind> = std::iter::once(*zone)
            .chain(also.iter().copied())
            .filter(|kind| *kind != ZoneKind::Library || !cant_search(state, player))
            .collect();
        if named.is_empty() {
            continue;
        }
        let searched = match zones_searched(&mut asked, player, &named) {
            Ok(searched) => searched,
            Err(stop) => return Stop::settle(Err(stop)),
        };
        let mut places = Vec::new();
        let mut matches: Vec<ObjectId> = Vec::new();
        for kind in &searched {
            let place = zone_of(*kind, player)?;
            places.push(place);
            for id in state.zone(place) {
                if let Some(card) = state.object(*id)
                    && cx.scope.selects(filter, card.reference())?
                {
                    matches.push(*id);
                }
            }
        }
        // By id, which says nothing of where the cards lie in a library.
        matches.sort_unstable();
        let hidden = searched
            .iter()
            .any(|kind| matches!(kind, ZoneKind::Library | ZoneKind::Hand));
        let fewest = if hidden { 0 } else { required };
        let take = most.min(matches.len());
        let cards = if take == 0 {
            Vec::new()
        } else if matches.len() <= fewest {
            matches
        } else {
            let text = wording::search_zones(&searched, filter, required, most);
            match asked.cards(player, matches, (fewest, take.max(fewest)), text) {
                Ok(cards) => cards,
                Err(stop) => return Stop::settle(Err(stop)),
            }
        };
        found.extend(
            cards
                .iter()
                .filter_map(|id| state.object(*id))
                .map(|card| Target::Object(card.reference())),
        );
        for place in places {
            let here: Vec<ObjectId> = cards
                .iter()
                .copied()
                .filter(|id| state.object(*id).is_some_and(|card| card.zone() == place))
                .collect();
            events.push(Event::CardsFound(CardsFound {
                player,
                zone: place,
                cards: here,
            }));
        }
        if *reveal && !cards.is_empty() {
            events.push(Event::Revealed(Revealed { player, cards }));
        }
    }
    asked.end()?;
    Ok(Run::Done {
        events,
        bound: vec![(bind.clone(), found)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// The zones `player` searches among those they may: the one there is, or the choice they are asked for.
fn zones_searched(
    asked: &mut Questions<'_>,
    player: PlayerId,
    named: &[ZoneKind],
) -> Result<Vec<ZoneKind>, Stop> {
    if named.len() < 2 {
        return Ok(named.to_vec());
    }
    let subsets = choices(named);
    let options = subsets
        .iter()
        .map(|subset| ChoiceOption::Label(labelled(subset)))
        .collect();
    let text = format!("Which of {} do you search?", wording::search_places(named));
    let at = asked.w9b_option(player, options, text)?;
    subsets
        .into_iter()
        .nth(at)
        .ok_or_else(|| Stop::Fail(ResolveError::Invalid("no zones were chosen".into())))
}

#[cfg(test)]
mod tests {
    //! Goal: the choice of zones to search lists every non-empty set of the zones a card names, the whole set first and the single zones last, in the order the card names them, with a label that reads as the words of the choice.
    use ZoneKind::{Graveyard, Hand, Library};
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::one(vec![Library], vec![vec![Library]])]
    #[case::two(vec![Library, Hand], vec![vec![Library, Hand], vec![Library], vec![Hand]])]
    #[case::three(
        vec![Library, Hand, Graveyard],
        vec![
            vec![Library, Hand, Graveyard],
            vec![Library, Hand],
            vec![Library, Graveyard],
            vec![Hand, Graveyard],
            vec![Library],
            vec![Hand],
            vec![Graveyard],
        ]
    )]
    #[case::another_order(
        vec![Graveyard, Library],
        vec![vec![Graveyard, Library], vec![Graveyard], vec![Library]]
    )]
    fn every_set_of_the_zones_is_a_choice(
        #[case] zones: Vec<ZoneKind>,
        #[case] expected: Vec<Vec<ZoneKind>>,
    ) {
        assert_eq!(choices(&zones), expected);
    }

    #[rstest]
    #[case::one(vec![Hand], "Hand")]
    #[case::two(vec![Library, Graveyard], "Library and graveyard")]
    #[case::three(vec![Library, Hand, Graveyard], "Library, hand and graveyard")]
    fn a_choice_reads_as_words(#[case] zones: Vec<ZoneKind>, #[case] expected: &str) {
        assert_eq!(labelled(&zones), expected);
    }
}
