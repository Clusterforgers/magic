//! Paying to ignore an effect: "Any player may pay {2} for that player to ignore this effect until end of turn."
//! (`GameRule::PayToIgnore`), or a named payer and a cost that is not mana: "Its controller may exile three cards from
//! their graveyard for that player to ignore this effect until end of turn." (`GameRule::PayToIgnoreBy`). Paying is a
//! special action of a player with priority that does not use the stack (CR 116.2); the payer is logged with the
//! source, and the source's effects leave them out until end of turn through `Condition::PaidToIgnore` or
//! `Condition::PaidToIgnoreBy`.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::choose::Choices;
use crate::decision::PaidIgnore;
use crate::events::Event;
use crate::mana::{ManaOption, ManaPool, can_afford};
use crate::mana_takes::for_card;
use crate::parts::{Part, Payer};
use crate::price::Price;
use crate::spend::Spend;
use crate::state::GameState;

/// What ignoring a source costs `player`: the mana and the parts besides mana, in the order the cost prints them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IgnorePrice {
    /// The mana.
    pub(crate) mana: ManaCost,
    /// The other parts ("Sacrifice a permanent", "Exile three cards from your graveyard").
    pub(crate) parts: Vec<Part>,
    /// The cost as words for a line that is not only mana.
    pub(crate) words: Option<String>,
}

/// The price for `player` to ignore the effects of `source`, while its rule is in force, names them as a payer and they
/// have not paid this turn. A cost the special action cannot pay (tapping, `{X}`, the source leaving) is none.
pub(crate) fn offer(state: &GameState, player: PlayerId, source: ObjectId) -> Option<IgnorePrice> {
    let found = crate::rules::pay_to_ignore(state).iter().find(|found| {
        found.source.id == source
            && found
                .payers
                .as_ref()
                .is_none_or(|payers| payers.contains(&player))
            && !state.r6703_ignored.contains(&(player, found.source))
    })?;
    let price = Price::announced(&found.cost).ok()?;
    if price.tap
        || price.untap
        || price.x > 0
        || price.moves_source()
        || price.has_waterbend()
        || !price.changes.is_empty()
        || price
            .parts
            .iter()
            .any(|part| matches!(part, Part::Unattach | Part::X(_) | Part::Among { .. }))
    {
        return None;
    }
    let words = (!price.parts.is_empty())
        .then(|| mtg_cards::compile::render_cost(&found.cost))
        .flatten();
    Some(IgnorePrice {
        mana: price.mana.unwrap_or_default(),
        parts: price.parts,
        words,
    })
}

/// Who pays the parts of the price of ignoring `source`; the parts may take the source itself.
pub(crate) fn payer(state: &GameState, player: PlayerId, source: ObjectId) -> Payer<'_> {
    Payer {
        state,
        player,
        source,
        barred: &[],
        granter: None,
        pays: crate::r4707_pay_costs::PaysFor::Effect,
    }
}

/// Whether `player` has what the parts of the price take: enough objects for each pick, and the life and counters of
/// the others.
fn parts_payable(state: &GameState, player: PlayerId, source: ObjectId, parts: &[Part]) -> bool {
    let owes = payer(state, player, source);
    parts.iter().all(|part| match part {
        Part::Pick(pick) => !pick.short(state, &pick.candidates(&owes)),
        _ => part.payable(&owes, &[]).is_some(),
    })
}

/// The sources whose effects `player` may ignore now by paying, with the price, when they can pay it.
pub(crate) fn ignorable(
    state: &GameState,
    player: PlayerId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> Vec<PaidIgnore> {
    let mut found: Vec<PaidIgnore> = Vec::new();
    for rule in crate::rules::pay_to_ignore(state) {
        let source = rule.source.id;
        if found.iter().any(|offered| offered.object == source) {
            continue;
        }
        let Some(price) = offer(state, player, source) else {
            continue;
        };
        if parts_payable(state, player, source, &price.parts)
            && can_afford(
                &price.mana,
                pool,
                &for_card(state, sources, source),
                life,
                &Spend::other(),
            )
        {
            found.push(PaidIgnore {
                object: source,
                cost: price.mana,
                words: price.words,
            });
        }
    }
    found
}

/// The events of `player` ignoring the effects of `source`: those of the parts of the price with the objects picked
/// for them in `choices`, then the ignoring itself; none when they may not now.
pub(crate) fn events(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    choices: &Choices,
) -> Option<Vec<Event>> {
    let price = offer(state, player, source)?;
    let mut events = crate::parts::pay(
        state,
        player,
        source,
        &price.parts,
        choices.cost_picks(),
        &[],
    )?;
    let source = state.object(source)?.reference();
    events.push(Event::IgnoredEffect { player, source });
    Some(events)
}
