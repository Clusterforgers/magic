//! `ReplaceBy::LifeFloor` (CR 614.1a): damage that would reduce a player's life total below a number reduces it to that
//! number instead ("Damage that would reduce your life total to less than 1 reduces it to 1 instead", Worship). The
//! player's life is read as it stands after the damage and life loss of the batch before this one, so two damage events of one combat
//! damage step share the room above the floor.

use mtg_core::ids::PlayerId;

use super::{Item, Origin, Pass, Slot};
use crate::combat::DamageTarget;
use crate::events::Event;

impl Pass<'_> {
    /// Lowers the damage at `at` so that the player it hits keeps `floor` life, when they have at least that much and the
    /// damage would take them below it; any other damage is left as it is.
    pub(super) fn r1f_life_floor(&mut self, at: usize, floor: u32, applied: Vec<Origin>) {
        let Item::One(Event::DamageDealt {
            source,
            target: DamageTarget::Player(player),
            amount,
            deathtouch,
            form,
            toxic,
        }) = self.slots[at].item.clone()
        else {
            self.slots[at].applied = applied;
            return;
        };
        let life = self.r1f_life_before(at, player);
        let floor = i64::from(floor);
        let now = if life >= floor && life - i64::from(amount) < floor {
            u32::try_from(life - floor).unwrap_or(0)
        } else {
            amount
        };
        // The lifelink gain of the source stays whole: the damage is dealt in full, only the life lost is less.
        if now == 0 {
            self.slots.remove(at);
            return;
        }
        self.slots[at] = Slot {
            item: Item::One(Event::DamageDealt {
                source,
                target: DamageTarget::Player(player),
                amount: now,
                deathtouch,
                form,
                toxic,
            }),
            applied,
            frozen: false,
        };
    }

    /// The life of `player` once the events before `at` are applied.
    fn r1f_life_before(&self, at: usize, player: PlayerId) -> i64 {
        let mut life = self
            .state
            .player(player)
            .map_or(0, |held| i64::from(held.life()));
        for slot in &self.slots[..at] {
            match &slot.item {
                Item::One(Event::DamageDealt {
                    target: DamageTarget::Player(hit),
                    amount,
                    ..
                }) if *hit == player => life -= i64::from(*amount),
                Item::One(Event::LifeLost {
                    player: lost,
                    amount,
                }) if *lost == player => {
                    life -= i64::from(*amount);
                }
                _ => {}
            }
        }
        life
    }
}

#[cfg(test)]
mod tests {
    //! Goal: damage that would take its player below the floor is cut to leave exactly the floor (CR 614.1a), damage
    //! to a player already below it, to another player or within the room is dealt whole, and the damage events of one
    //! batch share the room; at two, three and four seats.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::super::{Processed, process};
    use crate::combat::{DamageForm, DamageTarget};
    use crate::events::Event;
    use crate::fixtures::{creature, setup};
    use crate::state::GameState;
    use crate::zones::Zone;

    const FLOOR: &str = r#"{"Replacement":{"when":{"Damage":{"to":"You"}},"by":{"LifeFloor":1}}}"#;

    /// Seat 0 controls the floor and has `life`; seat 1 controls a Bear; the other seats are empty.
    fn game(seats: usize, life: i32) -> GameState {
        let rule: Ability = serde_json::from_str(FLOOR).unwrap();
        let source = CardDefinition::new("Worship")
            .with_type(CardType::Enchantment)
            .with_rules(rule);
        let mut battlefields = vec![vec![Arc::new(source)], vec![creature("Bear", 2, 2)]];
        battlefields.resize(seats, Vec::new());
        let mut game = setup(vec![vec![]; seats], battlefields);
        game.players[0].life = Some(life);
        GameState::from_setup(&game).unwrap()
    }

    fn hit(game: &GameState, to: usize, amount: u32) -> Event {
        Event::DamageDealt {
            source: game.zone(Zone::Battlefield)[1],
            target: DamageTarget::Player(PlayerId(to.try_into().unwrap())),
            amount,
            deathtouch: false,
            form: DamageForm::default(),
            toxic: 0,
        }
    }

    fn dealt(game: &GameState, batch: Vec<Event>) -> Vec<u32> {
        let Processed::Events(out) = process(game, batch, &[]).unwrap() else {
            panic!("nothing is asked");
        };
        out.iter()
            .filter_map(|event| match event {
                Event::DamageDealt { amount, .. } => Some(*amount),
                _ => None,
            })
            .collect()
    }

    #[rstest]
    #[case::room(20, 3, vec![3])]
    #[case::exactly_to_the_floor(4, 3, vec![3])]
    #[case::cut(5, 7, vec![4])]
    #[case::cut_to_nothing(1, 3, vec![])]
    #[case::already_below(0, 3, vec![3])]
    fn damage_leaves_the_floor(
        #[values(2, 3, 4)] seats: usize,
        #[case] life: i32,
        #[case] amount: u32,
        #[case] want: Vec<u32>,
    ) {
        let game = game(seats, life);
        assert_eq!(dealt(&game, vec![hit(&game, 0, amount)]), want);
    }

    #[rstest]
    fn damage_to_another_player_is_dealt(#[values(2, 3, 4)] seats: usize) {
        let game = game(seats, 2);
        assert_eq!(dealt(&game, vec![hit(&game, 1, 9)]), [9]);
    }

    #[rstest]
    fn damage_events_of_one_batch_share_the_room(#[values(2, 3)] seats: usize) {
        let game = game(seats, 4);
        let batch = vec![hit(&game, 0, 3), hit(&game, 0, 3)];
        assert_eq!(dealt(&game, batch), [3]);
    }
}
