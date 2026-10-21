//! Protection (CR 702.16): a permanent or player with protection from a quality can't be damaged, enchanted,
//! equipped, fortified, blocked or targeted by a source with that quality (DEBT). The quality is a selector over the
//! source, so a colour, a card type, a player's objects and everything are all the same rule.

use mtg_cards::keyword::Keyword;

use super::{Holder, KeywordRules, Source};
use crate::characteristics::controller_of_object;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// The rules of protection.
#[derive(Debug)]
pub(super) struct Protection;

impl KeywordRules for Protection {
    /// The quality is judged for the holder's controller (so "your opponents" is the holder's opponents), against the
    /// source as it was if it has gone (last-known information, CR 608.2h). Protection from everything has the empty
    /// selector and needs no look at the source.
    fn protects_from(
        &self,
        state: &GameState,
        keyword: &Keyword,
        holder: Holder,
        source: Source<'_>,
    ) -> bool {
        let Keyword::Protection(quality) = keyword else {
            return false;
        };
        let (controller, anchor) = match holder {
            Holder::Object(id) => {
                // A static ability works on the battlefield only (CR 113.6).
                let Some(object) = state
                    .object(id)
                    .filter(|object| object.zone() == Zone::Battlefield)
                else {
                    return false;
                };
                (controller_of_object(state, object), object.reference())
            }
            Holder::Player(player) => (player, source.object),
        };
        if quality.is_empty() {
            return true;
        }
        let mut scope = Scope::announcing(state, controller, anchor);
        scope.snapshots = source.snapshots.to_vec();
        scope.selects(quality, source.object).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: protection is judged for the holder's controller against the source as it is or, once the source has changed zones, as its snapshot says; the empty quality protects from everything, a colour or a card type only from sources that have it, a permanent holds it on the battlefield alone, and a player holds it from their setup.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_cards::refs::{Pred, Ref, Selector};
    use mtg_core::card::CardType;
    use mtg_core::color::Color;
    use mtg_core::ids::{ObjectId, PlayerId};
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::keywords::protected_from;
    use crate::snapshot::snapshot;
    use crate::state::GameState;

    fn imp() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Imp")
                .with_type(CardType::Creature)
                .with_mana_cost(ManaCost::parse("{R}").unwrap())
                .with_power_toughness(1, 1),
        )
    }

    fn guard(quality: Selector) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Guard")
                .with_type(CardType::Creature)
                .with_power_toughness(2, 2)
                .with_rules(Ability::Keyword(Keyword::Protection(quality))),
        )
    }

    /// Seat 0 controls a red Imp, seat 1 a Guard with the quality and a colourless Bear: the Imp, the Guard and the Bear.
    fn table(quality: Selector) -> (GameState, ObjectId, ObjectId, ObjectId) {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![imp()], vec![guard(quality), creature("Bear", 2, 2)]],
        );
        let on = game.zone(Zone::Battlefield).to_vec();
        (game, on[0], on[1], on[2])
    }

    #[rstest]
    #[case::a_colour_it_has(Selector(vec![Pred::Color(Color::Red)]), true, false)]
    #[case::a_colour_it_lacks(Selector(vec![Pred::Color(Color::Blue)]), false, false)]
    #[case::a_card_type(Selector(vec![Pred::Type(CardType::Creature)]), true, true)]
    #[case::a_card_type_it_lacks(Selector(vec![Pred::Type(CardType::Artifact)]), false, false)]
    #[case::the_opponents_of_the_holder(Selector(vec![Pred::ControlledBy(Ref::Opponents)]), true, false)]
    #[case::everything(Selector::default(), true, true)]
    fn the_quality_decides_which_sources_are_stopped(
        #[case] quality: Selector,
        #[case] imp_stopped: bool,
        #[case] bear_stopped: bool,
    ) {
        let (game, imp, guard, bear) = table(quality);
        let from = |id| Source::live(&game, id).unwrap();
        let holder = Holder::Object(guard);
        assert_eq!(protected_from(&game, holder, from(imp)), imp_stopped);
        assert_eq!(protected_from(&game, holder, from(bear)), bear_stopped);
    }

    #[test]
    fn a_source_that_has_gone_is_read_as_its_snapshot_says() {
        let (mut game, imp, guard, _) = table(Selector(vec![Pred::Color(Color::Red)]));
        let shot = snapshot(&game, imp).unwrap();
        let named = Source::live(&game, imp).unwrap().object;
        // A new object now: the old reference names nothing, and the snapshot is all that is known of it.
        let gone = game.objects.get_mut(&imp).unwrap();
        gone.counter += 1;
        gone.zone = Zone::Graveyard(PlayerId(0));
        let holder = Holder::Object(guard);
        let without = Source {
            object: named,
            snapshots: &[],
        };
        assert!(!protected_from(&game, holder, without));
        let known = [&shot];
        let with = Source {
            object: named,
            snapshots: &known,
        };
        assert!(protected_from(&game, holder, with));
    }

    #[test]
    fn protection_works_on_the_battlefield_only() {
        let (mut game, imp, guard, _) = table(Selector::default());
        let from = Source::live(&game, imp).unwrap();
        assert!(protected_from(&game, Holder::Object(guard), from));
        game.objects.get_mut(&guard).unwrap().zone = Zone::Graveyard(PlayerId(1));
        assert!(!protected_from(&game, Holder::Object(guard), from));
    }

    #[test]
    fn a_player_holds_protection_from_their_setup() {
        let mut setup = crate::fixtures::setup(vec![vec![], vec![]], vec![vec![imp()], vec![]]);
        let red = Selector(vec![Pred::Color(Color::Red)]);
        setup.players[1].keywords = vec![Keyword::Protection(red)];
        let game = GameState::from_setup(&setup).unwrap();
        let imp = game.zone(Zone::Battlefield)[0];
        let from = Source::live(&game, imp).unwrap();
        assert!(protected_from(&game, Holder::Player(PlayerId(1)), from));
        assert!(!protected_from(&game, Holder::Player(PlayerId(0)), from));
        assert!(game.possible.protection);
    }
}
