//! The boss on the fight's intro: its innate with its level, the encounter modifier, its relics, its two phases with
//! their rounds, its Satchel with each card's reusable mark and its personality, from the plan its game is made with.

use mtg_campaign::FightPlan;
use mtg_campaign::content::Content;
use mtg_engine::boss_sheet::SheetCard;
use mtg_protocol::rest::DelveFightView;
use mtg_protocol::view::{BossPartView, BossSatchelView, InnateView, RoundPhaseView};

/// The intro of the fight `plan`; none when the content lacks one of its cards.
pub(crate) fn fight_view(plan: &FightPlan, content: &Content) -> Option<DelveFightView> {
    let boss = &plan.boss;
    let sheet = boss.sheet(content).ok()?;
    let part = |card: &SheetCard| BossPartView {
        name: card.name.clone(),
        text: card.text.clone(),
    };
    let innate = sheet.innate.as_ref()?;
    Some(DelveFightView {
        boss: boss.name.clone(),
        life: boss.life,
        innate: InnateView {
            name: innate.name.clone(),
            text: innate.text.clone(),
            level: sheet.level,
        },
        modifier: sheet.modifier.as_ref().map(part),
        relics: sheet.relics.iter().map(part).collect(),
        round_phases: sheet
            .phases
            .iter()
            .zip(&boss.round_phases)
            .map(|(card, (_, round))| RoundPhaseView {
                name: card.name.clone(),
                text: card.text.clone(),
                round: *round,
                begun: false,
            })
            .collect(),
        satchel: boss
            .satchel
            .iter()
            .map(|card| BossSatchelView {
                name: card.card.clone(),
                reusable: card.reusable,
                tax: 0,
                object: None,
            })
            .collect(),
        personality: Some(BossPartView {
            name: boss.personality.name().to_owned(),
            text: boss.personality.text().to_owned(),
        }),
    })
}
