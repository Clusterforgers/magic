//! The reworked boss's seat (content generation [`REWORK_FROM`] on): its innate at the level of the run's depth and
//! Heat, one encounter modifier and its relics drawn by the seed keyed by floor and room, two phases by round, its
//! satchel, its life by party size only, and the Hard brain. No scheme, minion, commander card or Enrage.

use mtg_engine::boss_sheet::{BossSheet, SheetCard};

use crate::CampaignError;
use crate::boss_relics;
use crate::content::{Boss, Brain, Content};
use crate::fight::{BossSeat, HAND, Reworked};
use crate::innate::innate_id;
use crate::run::Run;
use crate::scale::{PHASE_ROUNDS, REWORK_FROM, Scale};

impl Run {
    /// The bosses the party defeated in this run over all floors: the depth of its next boss.
    pub fn defeated(&self) -> u32 {
        u32::try_from(self.fights.iter().filter(|f| f.won).count()).unwrap_or(u32::MAX)
    }

    /// Whether the run plays the reworked boss.
    pub fn reworked(&self) -> bool {
        self.content >= REWORK_FROM
    }

    /// The reworked seat of `boss` in room `node` against `heroes` heroes at `depth`.
    pub fn reworked_seat(
        &self,
        content: &Content,
        boss: &Boss,
        node: u8,
        (heroes, depth): (usize, u32),
    ) -> BossSeat {
        let level = Scale::innate_level(depth, self.heat);
        let modifier =
            crate::modifiers::draw(&content.modifiers, &boss.id, self.seed, self.floor, node);
        let makeup = boss.makeup.clone().unwrap_or_default();
        let relics = boss_relics::draw(
            content,
            &makeup,
            self.content,
            (depth, self.heat),
            (self.seed, self.floor, node),
        );
        BossSeat {
            boss: boss.id.clone(),
            name: boss.name.clone(),
            life: Scale::boss_life(heroes),
            hand: HAND,
            brain: Brain::Hard,
            deck: boss.deck.clone(),
            commander: None,
            rules: Vec::new(),
            phases: Vec::new(),
            schemes: Vec::new(),
            scheme_every: 1,
            rework: Some(Box::new(Reworked {
                innate: boss.innate.as_ref().map(|id| innate_id(id, level)),
                modifier: modifier.map(|m| m.id.clone()),
                relics,
                round_phases: boss
                    .round_phases
                    .iter()
                    .cloned()
                    .zip(PHASE_ROUNDS)
                    .collect(),
                satchel: boss.satchel.clone(),
            })),
        }
    }
}

impl Reworked {
    /// The public sheet the game names these parts by: each card's name and rules text, the innate's level.
    pub fn sheet(&self, content: &Content) -> Result<BossSheet, CampaignError> {
        let card = |id: &String| {
            content.card(id).map(|card| SheetCard {
                name: card.name.clone(),
                text: card.text.clone(),
            })
        };
        let innate = self
            .innate
            .as_ref()
            .map(|id| content.card(id))
            .transpose()?;
        Ok(BossSheet {
            level: innate.map_or(0, |card| card.level),
            innate: self.innate.as_ref().map(card).transpose()?,
            modifier: self.modifier.as_ref().map(card).transpose()?,
            relics: self.relics.iter().map(card).collect::<Result<_, _>>()?,
            phases: self
                .round_phases
                .iter()
                .map(|(id, _)| card(id))
                .collect::<Result<_, _>>()?,
        })
    }
}
