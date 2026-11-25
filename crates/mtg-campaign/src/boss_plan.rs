//! The boss's seat: its precon deck, its innate at the level of the run's depth and Heat, one encounter modifier and
//! its relics drawn by the seed keyed by floor and room, two phases by round, its satchel, its life and reinforcements
//! by party size only, and the Hard brain. A phase written with levels (as an innate is) plays at the innate's level.

use mtg_engine::boss_sheet::{BossSheet, SheetCard};

use crate::CampaignError;
use crate::boss_relics;
use crate::content::{Boss, Brain, Content};
use crate::fight::{BossSeat, HAND};
use crate::innate::innate_id;
use crate::run::Run;
use crate::scale::{PHASE_ROUNDS, Scale};

impl Run {
    /// The bosses the party defeated in this run over all floors: the depth of its next boss.
    pub fn defeated(&self) -> u32 {
        u32::try_from(self.fights.iter().filter(|f| f.won).count()).unwrap_or(u32::MAX)
    }

    /// The seat of `boss` in room `node` against `heroes` heroes at `depth`.
    pub fn boss_seat(
        &self,
        content: &Content,
        boss: &Boss,
        node: u8,
        (heroes, depth): (usize, u32),
    ) -> Result<BossSeat, CampaignError> {
        let level = Scale::innate_level(depth, self.heat);
        let modifier =
            crate::modifiers::draw(&content.modifiers, &boss.id, self.seed, self.floor, node);
        let deck = content.deck(&boss.deck_file)?;
        let relics = boss_relics::draw(
            content,
            &deck.makeup,
            self.content,
            (depth, self.heat),
            (self.seed, self.floor, node),
        );
        Ok(BossSeat {
            boss: boss.id.clone(),
            name: boss.name.clone(),
            life: Scale::boss_life(heroes),
            hand: HAND,
            brain: Brain::Hard,
            deck: deck.cards.clone(),
            innate: innate_id(&boss.innate, level),
            modifier: modifier.map(|m| m.id.clone()),
            relics,
            reinforcements: Scale::theme_reinforcements(
                heroes,
                self.content,
                content
                    .themes
                    .iter()
                    .find(|t| t.boss(&boss.id).is_some())
                    .map_or(0, |t| t.boss_supplies),
                boss_relics::count(depth, self.heat, self.content),
            ),
            round_phases: boss
                .round_phases
                .iter()
                .map(|id| at_level(content, id, level))
                .zip(PHASE_ROUNDS)
                .collect(),
            satchel: boss.satchel.clone(),
        })
    }
}

/// The card of phase `id` at innate `level`: its level card when the phase is written with levels, else the phase as
/// written.
fn at_level(content: &Content, id: &str, level: u8) -> String {
    let leveled = innate_id(id, level);
    if content.cards.contains_key(&leveled) {
        leveled
    } else {
        id.to_owned()
    }
}

impl BossSeat {
    /// The public sheet the game names the boss's parts by: each card's name and rules text, the innate's level.
    pub fn sheet(&self, content: &Content) -> Result<BossSheet, CampaignError> {
        let card = |id: &String| {
            content.card(id).map(|card| SheetCard {
                name: card.name.clone(),
                text: card.text.clone(),
            })
        };
        Ok(BossSheet {
            level: content.card(&self.innate)?.level,
            innate: Some(card(&self.innate)?),
            modifier: self.modifier.as_ref().map(card).transpose()?,
            relics: self
                .relics
                .iter()
                .chain(&self.reinforcements)
                .map(card)
                .collect::<Result<_, _>>()?,
            phases: self
                .round_phases
                .iter()
                .map(|(id, _)| card(id))
                .collect::<Result<_, _>>()?,
        })
    }
}
