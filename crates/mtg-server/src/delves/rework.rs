//! The reworked boss on the fight's intro: its innate with its level, the encounter modifier, its relics, its two
//! phases with their rounds and its Satchel with each card's reusable mark, from the plan its game is made with.

use mtg_campaign::content::Content;
use mtg_campaign::fight::Reworked;
use mtg_engine::boss_sheet::SheetCard;
use mtg_protocol::rest::DelveFightView;
use mtg_protocol::view::{BossPartView, BossSatchelView, InnateView, RoundPhaseView};

/// `fight` with the parts of the reworked boss `rework`; as it was when the content lacks one of its cards.
pub(crate) fn with_rework(
    mut fight: DelveFightView,
    rework: &Reworked,
    content: &Content,
) -> DelveFightView {
    let Ok(sheet) = rework.sheet(content) else {
        return fight;
    };
    let part = |card: &SheetCard| BossPartView {
        name: card.name.clone(),
        text: card.text.clone(),
    };
    fight.innate = sheet.innate.as_ref().map(|card| InnateView {
        name: card.name.clone(),
        text: card.text.clone(),
        level: sheet.level,
    });
    fight.modifier = sheet.modifier.as_ref().map(part);
    fight.relics = sheet.relics.iter().map(part).collect();
    fight.round_phases = sheet
        .phases
        .iter()
        .zip(&rework.round_phases)
        .map(|(card, (_, round))| RoundPhaseView {
            name: card.name.clone(),
            text: card.text.clone(),
            round: *round,
            begun: false,
        })
        .collect();
    fight.satchel = rework
        .satchel
        .iter()
        .map(|card| BossSatchelView {
            name: card.card.clone(),
            reusable: card.reusable,
            tax: 0,
            object: None,
        })
        .collect();
    fight
}
