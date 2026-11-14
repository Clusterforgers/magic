//! The damage named objects dealt this turn to named players and objects (`Amount::DamageDealtByTo`), and the damage dealt to them by the sources of named players (`Amount::DamageToFromPlayers`), read off the
//! turn's damage records: "if it has dealt 10 or more damage to that player this turn".

use mtg_cards::value::TallyEvent;

use crate::choose::Target;
use crate::object::ObjectRef;
use crate::resolve::Scope;

impl Scope<'_> {
    /// The damage the `dealers` dealt this turn, after prevention, to what `to` names; a dealer that left and came back
    /// is another object.
    pub(crate) fn l294_damage_by_to(&self, dealers: &[ObjectRef], to: &[Target]) -> i64 {
        self.state
            .tallies
            .records
            .iter()
            .filter(|record| {
                record.event == TallyEvent::Damaged
                    && record
                        .subject
                        .is_some_and(|subject| dealers.contains(&subject))
                    && record.other.is_some_and(|other| to.contains(&other))
            })
            .map(|record| i64::from(record.amount))
            .sum()
    }

    /// The damage dealt this turn, after prevention, to what `to` names by sources while one of `players` controlled
    /// them (`Amount::DamageToFromPlayers`).
    pub(crate) fn r6008_damage_to_from(
        &self,
        to: &[Target],
        players: &[mtg_core::ids::PlayerId],
    ) -> i64 {
        self.state
            .tallies
            .records
            .iter()
            .filter(|record| {
                record.event == TallyEvent::Damaged
                    && record
                        .player
                        .is_some_and(|player| players.contains(&player))
                    && record.other.is_some_and(|other| to.contains(&other))
            })
            .map(|record| i64::from(record.amount))
            .sum()
    }
}
