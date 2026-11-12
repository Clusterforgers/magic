//! Teams: how the seats of a table play together (CR 808 team variants, CR 810 Two-Headed Giant).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::PlayerId;

/// How teams play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum TeamMode {
    /// Two teams of two with a shared turn and a shared life total (CR 810).
    TwoHeadedGiant,
    /// Teams of any size; each player takes their own turn and a team wins when all its opponents lost (CR 808).
    Teams,
}

/// The teams of a table: a mode and the team of every seat.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct Teams {
    /// How the teams play.
    pub mode: TeamMode,
    /// The team of each seat in table order, counted from 0.
    pub seats: Vec<u32>,
}

/// Why a team assignment cannot be played.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TeamError {
    /// The assignment names another number of seats than the table has.
    #[error("teams: {given} seats assigned at a table of {seats}")]
    SeatCount {
        /// The seats the assignment names.
        given: usize,
        /// The seats the table has.
        seats: usize,
    },
    /// Fewer than two teams play.
    #[error("teams: at least two teams play")]
    OneTeam,
    /// A team number is skipped, so a team has no seat.
    #[error("teams: team {0} has no seat")]
    EmptyTeam(u32),
    /// Two-Headed Giant is two teams of two.
    #[error("teams: Two-Headed Giant is two teams of two")]
    NotTwoByTwo,
}

impl Teams {
    /// Teams of `size` seats for `teams` teams; Two-Headed Giant seats teammates side by side (CR 810.3), other team
    /// games alternate the teams around the table.
    pub fn even(mode: TeamMode, teams: u32, size: u32) -> Self {
        let seats = (0..teams * size)
            .map(|seat| match mode {
                TeamMode::TwoHeadedGiant => seat / size,
                TeamMode::Teams => seat % teams,
            })
            .collect();
        Teams { mode, seats }
    }

    /// The team of `seat`; a seat the assignment lacks is a team of its own.
    pub fn team_of(&self, seat: PlayerId) -> u32 {
        self.seats
            .get(seat.index())
            .copied()
            .unwrap_or_else(|| u32::try_from(self.seats.len() + seat.index()).unwrap_or(u32::MAX))
    }

    /// The seats of `team`, in table order.
    pub fn members(&self, team: u32) -> impl Iterator<Item = PlayerId> + '_ {
        self.seats
            .iter()
            .enumerate()
            .filter(move |(_, t)| **t == team)
            .map(|(index, _)| PlayerId::from_index(index))
    }

    /// The number of teams.
    pub fn count(&self) -> u32 {
        self.seats.iter().max().map_or(0, |most| most + 1)
    }

    /// Whether the teams share turns and a life total.
    pub fn shared(&self) -> bool {
        self.mode == TeamMode::TwoHeadedGiant
    }

    /// Checks the assignment for a table of `seats`: every seat has a team, at least two teams play, no team number is
    /// skipped, and Two-Headed Giant is two teams of two.
    pub fn validate_for(&self, seats: usize) -> Result<(), TeamError> {
        if self.seats.len() != seats {
            return Err(TeamError::SeatCount {
                given: self.seats.len(),
                seats,
            });
        }
        let count = self.count();
        if count < 2 {
            return Err(TeamError::OneTeam);
        }
        if let Some(empty) = (0..count).find(|team| self.members(*team).next().is_none()) {
            return Err(TeamError::EmptyTeam(empty));
        }
        if self.mode == TeamMode::TwoHeadedGiant
            && (count != 2 || (0..2).any(|team| self.members(team).count() != 2))
        {
            return Err(TeamError::NotTwoByTwo);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Goal: even assignments seat Two-Headed Giant teammates together and alternate other teams, and the check refuses a wrong seat count, a single team, a skipped team number and a Two-Headed Giant that is not two by two.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::giant(TeamMode::TwoHeadedGiant, 2, 2, &[0, 0, 1, 1])]
    #[case::two_by_two(TeamMode::Teams, 2, 2, &[0, 1, 0, 1])]
    #[case::three_by_three(TeamMode::Teams, 2, 3, &[0, 1, 0, 1, 0, 1])]
    #[case::three_teams(TeamMode::Teams, 3, 2, &[0, 1, 2, 0, 1, 2])]
    fn even_assignments(
        #[case] mode: TeamMode,
        #[case] teams: u32,
        #[case] size: u32,
        #[case] seats: &[u32],
    ) {
        let got = Teams::even(mode, teams, size);
        assert_eq!(got.seats, seats);
        assert_eq!(got.validate_for(seats.len()), Ok(()));
        assert_eq!(got.count(), teams);
    }

    #[rstest]
    #[case::short(TeamMode::Teams, &[0, 1], 3, Some(TeamError::SeatCount { given: 2, seats: 3 }))]
    #[case::one_team(TeamMode::Teams, &[0, 0, 0], 3, Some(TeamError::OneTeam))]
    #[case::skipped(TeamMode::Teams, &[0, 2, 0, 2], 4, Some(TeamError::EmptyTeam(1)))]
    #[case::uneven(TeamMode::Teams, &[0, 0, 1], 3, None)]
    #[case::giant_of_three(TeamMode::TwoHeadedGiant, &[0, 0, 0, 1], 4, Some(TeamError::NotTwoByTwo))]
    #[case::giant_of_six(TeamMode::TwoHeadedGiant, &[0, 0, 1, 1, 2, 2], 6, Some(TeamError::NotTwoByTwo))]
    #[case::giant_apart(TeamMode::TwoHeadedGiant, &[0, 1, 0, 1], 4, None)]
    fn the_check(
        #[case] mode: TeamMode,
        #[case] seats: &[u32],
        #[case] table: usize,
        #[case] error: Option<TeamError>,
    ) {
        let teams = Teams {
            mode,
            seats: seats.to_vec(),
        };
        assert_eq!(teams.validate_for(table).err(), error);
    }

    #[test]
    fn members_and_strays() {
        let teams = Teams::even(TeamMode::Teams, 2, 2);
        assert_eq!(
            teams.members(1).collect::<Vec<_>>(),
            [PlayerId::from_index(1), PlayerId::from_index(3)]
        );
        assert_eq!(teams.team_of(PlayerId::from_index(2)), 0);
        assert_eq!(teams.team_of(PlayerId::from_index(5)), 9);
    }
}
