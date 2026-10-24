//! Divided amounts at resolution: what each target of a divided spell or ability receives (CR 601.2d, 608.2b).
//!
//! `Divided` is a recipient's share of the amount the controller divided as the spell or ability was announced
//! (`Choices.division`); `EvenShare` is the whole split evenly, rounded down, among the recipients that are still
//! legal. Damage and counters read them one recipient at a time (`Scope::amounts_for`); no other value has a recipient.

use mtg_cards::value::Amount;

use super::value::{ResolveError, Scope};
use crate::choose::Target;

/// Why a divided amount cannot be read: it is the share of a recipient, and only an effect that gives one has recipients.
pub(super) const OUTSIDE_A_DIVISION: &str =
    "a divided amount outside the damage or counters it divides";

/// Whose share a divided amount reads: the recipient of the effect that gives out the amount, and how many recipients
/// the effect names.
#[derive(Debug, Clone, Copy)]
pub(super) struct Share {
    pub(super) recipient: Target,
    pub(super) among: usize,
}

/// Whether the amount reads a division, directly or in a sum (the places `Divided` and `EvenShare` are allowed).
fn reads_division(amount: &Amount) -> bool {
    match amount {
        Amount::Divided | Amount::EvenShare(_) => true,
        Amount::Sum(parts) => parts.iter().any(reads_division),
        Amount::Times { of, .. } | Amount::Half { of, .. } => reads_division(of),
        _ => false,
    }
}

impl Scope<'_> {
    /// What each of `recipients` gets of an amount, in their order: the same for all unless the amount reads a
    /// division, which gives a recipient its own share (`Divided`) or an even part of the whole (`EvenShare`).
    /// `recipients` are the legal targets the effect names, so a target that became illegal has no share and the
    /// others keep theirs (CR 608.2b).
    pub(crate) fn amounts_for(
        &self,
        amount: &Amount,
        recipients: &[Target],
    ) -> Result<Vec<i64>, ResolveError> {
        if !reads_division(amount) {
            let each = self.amount(amount)?;
            return Ok(vec![each; recipients.len()]);
        }
        recipients
            .iter()
            .map(|recipient| {
                let share = Share {
                    recipient: *recipient,
                    among: recipients.len(),
                };
                self.amount_of(amount, Some(share))
            })
            .collect()
    }

    /// This recipient's share of what was divided as the spell or ability was announced; none when it was given nothing.
    pub(super) fn divided_share(&self, recipient: Target) -> i64 {
        self.choices
            .division
            .iter()
            .find(|(target, _)| *target == recipient)
            .map_or(0, |(_, share)| i64::from(*share))
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a divided amount gives each recipient its own share or an even part, rounded down, of the whole among the recipients named; what the division left out gets nothing, a sum adds the share to the rest, an amount that reads no division is the same for all, and a divided amount read with no recipient is refused by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::Choices;
    use crate::fixtures::{creature, state};
    use crate::zones::Zone;

    fn player(seat: u8) -> Target {
        Target::Player(PlayerId(seat.into()))
    }

    /// What each recipient (seats) gets of the amount when the division gives each seat its share.
    fn amounts(json: &str, division: &[(u8, u32)], recipients: &[u8]) -> Vec<i64> {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        let id = game.zone(Zone::Battlefield)[0];
        let source = game.object(id).unwrap().reference();
        let choices = Choices {
            division: division
                .iter()
                .map(|(seat, share)| (player(*seat), *share))
                .collect(),
            ..Choices::default()
        };
        let mut scope = Scope::announcing(&game, PlayerId(0), source);
        scope.choices = &choices;
        let recipients: Vec<Target> = recipients.iter().map(|seat| player(*seat)).collect();
        let amount: Amount = serde_json::from_str(json).unwrap();
        scope.amounts_for(&amount, &recipients).unwrap()
    }

    #[rstest]
    #[case::each_its_own(r#""Divided""#, &[(0, 2), (1, 1)], &[0, 1], &[2, 1])]
    #[case::in_the_order_of_the_recipients(r#""Divided""#, &[(0, 2), (1, 1)], &[1, 0], &[1, 2])]
    #[case::a_recipient_the_division_left_out_gets_nothing(r#""Divided""#, &[(0, 2)], &[0, 1], &[2, 0])]
    #[case::the_share_of_a_target_that_is_no_recipient_goes_to_nobody(r#""Divided""#, &[(0, 2), (1, 1)], &[1], &[1])]
    #[case::the_share_and_one_more(r#"{"Sum":["Divided",1]}"#, &[(0, 2), (1, 1)], &[0, 1], &[3, 2])]
    #[case::the_whole_to_one(r#""Divided""#, &[(1, 4)], &[1], &[4])]
    #[case::evenly_among_two(r#"{"EvenShare":5}"#, &[], &[0, 1], &[2, 2])]
    #[case::rounded_down_among_three(r#"{"EvenShare":5}"#, &[], &[0, 1, 1], &[1, 1, 1])]
    #[case::exactly(r#"{"EvenShare":6}"#, &[], &[0, 1, 1], &[2, 2, 2])]
    #[case::the_whole_to_one_recipient(r#"{"EvenShare":5}"#, &[], &[1], &[5])]
    #[case::fewer_than_the_recipients(r#"{"EvenShare":2}"#, &[], &[0, 1, 1], &[0, 0, 0])]
    #[case::of_less_than_nothing(r#"{"EvenShare":{"Sum":[1,-4]}}"#, &[], &[0, 1], &[0, 0])]
    #[case::nobody_to_give_it_to(r#"{"EvenShare":5}"#, &[], &[], &[])]
    #[case::a_fixed_amount_is_the_same_for_all(r"4", &[(0, 1)], &[0, 1], &[4, 4])]
    fn what_each_recipient_gets_of_the_amount(
        #[case] json: &str,
        #[case] division: &[(u8, u32)],
        #[case] recipients: &[u8],
        #[case] expected: &[i64],
    ) {
        assert_eq!(amounts(json, division, recipients), expected);
    }
}
