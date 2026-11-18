//! A boss seat's public view: its phases, scheme and Omen, and a reworked boss's sheet (innate, modifier, relics),
//! phases by round with the countdown to the next, and its Satchel with each card's reusable mark and tax.

use mtg_engine::boss_sheet::{BossSheet, SheetCard};
use mtg_engine::phases::PhaseStart;
use mtg_engine::player::Player;
use mtg_engine::state::GameState;

use crate::view::{BossPartView, BossSatchelView, BossView, InnateView, RoundPhaseView};

/// The boss view of `player`; none for a seat with no phases, schemes or sheet.
pub(crate) fn of(state: &GameState, player: &Player) -> Option<BossView> {
    let seat = player.id();
    let sheet = player.boss_sheet();
    let ((count, begun), schemes) = (player.phases(), player.schemes());
    if count == 0 && schemes.is_none() && sheet.is_none() {
        return None;
    }
    let round_phases = round_phases(player, sheet);
    let current = state.turn().round().max(1);
    let phase_in = round_phases
        .iter()
        .find(|phase| !phase.begun)
        .map(|phase| phase.round.saturating_add(1).saturating_sub(current).max(1));
    Some(BossView {
        phases: player.phase_names().map(str::to_owned).collect(),
        begun: u32::try_from(begun).unwrap_or(u32::MAX),
        commander: player.held().next().map(str::to_owned),
        scheme: schemes.and_then(|(now, _)| now).map(str::to_owned),
        omen: schemes.map(|(_, omen)| omen.to_owned()),
        omen_in: mtg_engine::schemes::omen_in(state, seat),
        innate: sheet.and_then(|sheet| {
            sheet.innate.as_ref().map(|card| InnateView {
                name: card.name.clone(),
                text: card.text.clone(),
                level: sheet.level,
            })
        }),
        modifier: sheet.and_then(|sheet| sheet.modifier.as_ref().map(part)),
        round_phases,
        phase_in,
        relics: sheet.map_or_else(Vec::new, |sheet| sheet.relics.iter().map(part).collect()),
        satchel: state
            .satchel(seat)
            .into_iter()
            .filter_map(|id| {
                let card = state.object(id)?;
                Some(BossSatchelView {
                    name: card.card().name.clone(),
                    reusable: state.satchel_reusable(id),
                    tax: state.satchel_tax(id),
                    object: Some(id),
                })
            })
            .collect(),
    })
}

/// A sheet card as the view names it.
fn part(card: &SheetCard) -> BossPartView {
    BossPartView {
        name: card.name.clone(),
        text: card.text.clone(),
    }
}

/// The player's phases that begin by round, each with its words from the sheet and whether it has begun.
fn round_phases(player: &Player, sheet: Option<&BossSheet>) -> Vec<RoundPhaseView> {
    let (_, begun) = player.phases();
    player
        .boss_phases()
        .iter()
        .enumerate()
        .filter_map(|(at, phase)| match phase.begins {
            PhaseStart::AtRound(round) => Some(RoundPhaseView {
                name: phase.emblem.name.clone(),
                text: sheet
                    .map(|sheet| sheet.phase_text(&phase.emblem.name).to_owned())
                    .unwrap_or_default(),
                round,
                begun: at < begun,
            }),
            PhaseStart::AtLife(_) => None,
        })
        .collect()
}
