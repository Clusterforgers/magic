//! "If you would draw a card except the first one you draw in each of your draw steps": the draws of a draw step that
//! such a replacement leaves alone (CR 614.1a, 504.1).

use mtg_cards::trigger::ReplaceWhen;

use super::{Item, Pass, Unit};
use crate::events::Event;
use crate::state::Tally;
use crate::turn::Step;

impl Pass<'_> {
    /// Whether the draw at slot `at` is one `when` leaves alone: one of the first `except_draw_step` cards the active
    /// player draws in the draw step under way, counting the draws before it in this batch.
    pub(super) fn c6_blk_spared_draw(&self, at: usize, when: &ReplaceWhen, unit: &Unit) -> bool {
        let ReplaceWhen::Draw {
            except_draw_step, ..
        } = when
        else {
            return false;
        };
        let Unit::Draw { player } = unit else {
            return false;
        };
        if *except_draw_step == 0
            || self.state.turn.step != Step::Draw
            || self.state.active_player() != Some(*player)
        {
            return false;
        }
        let earlier = self.slots[..at]
            .iter()
            .filter(|slot| {
                matches!(&slot.item, Item::One(Event::CardDrawn { player: who, .. } | Event::DrewFromEmptyLibrary { player: who }) if who == player)
            })
            .count();
        let drawn = self.state.tallied(*player, Tally::C6BlkDrawStepDrawn);
        u32::try_from(earlier)
            .unwrap_or(u32::MAX)
            .saturating_add(drawn)
            < *except_draw_step
    }
}
