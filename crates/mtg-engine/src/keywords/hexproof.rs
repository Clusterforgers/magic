//! Hexproof (CR 702.11): a permanent or player with hexproof can't be the target of spells or abilities its controller's opponents control.

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};

use super::KeywordRules;
use crate::state::GameState;
use crate::zones::Zone;

/// The rules of hexproof.
#[derive(Debug)]
pub(super) struct Hexproof;

impl KeywordRules for Hexproof {
    fn restricts_targeting(
        &self,
        state: &GameState,
        _keyword: &Keyword,
        holder: ObjectId,
        _source: ObjectId,
        controller: PlayerId,
    ) -> bool {
        // A static ability works on the battlefield only (CR 113.6): a hexproof creature spell can be countered
        // and a hexproof card in a graveyard can be returned.
        state.object(holder).is_some_and(|object| {
            object.zone() == Zone::Battlefield
                && crate::characteristics::controller_of(state, holder) != Some(controller)
        })
    }

    fn restricts_player_targeting(
        &self,
        _keyword: &Keyword,
        holder: PlayerId,
        controller: PlayerId,
    ) -> bool {
        holder != controller
    }
}

#[cfg(test)]
mod tests {
    //! Goal: hexproof keeps the spells and abilities of the holder's opponents from targeting it and no one else's, for a permanent and for a player, at tables of two to four seats and on the battlefield only, and the recheck on resolution (CR 608.2b) drops a target that has become protected.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_cards::refs::{Ref, TargetSpec};
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;
    use crate::choose::{Choices, Target, is_legal, legal_targets};
    use crate::fixtures::{creature, setup};
    use crate::resolve::Scope;
    use crate::setup::GameSetup;

    fn shade() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Shade")
                .with_type(CardType::Creature)
                .with_power_toughness(2, 2)
                .with_rules(Ability::Keyword(Keyword::Hexproof)),
        )
    }

    fn spec(json: &str) -> TargetSpec {
        serde_json::from_str(json).unwrap()
    }

    /// `seats` players, each holding a vanilla "Spell" in hand; seat 1 controls a hexproof Shade and a Bear, seat 0 a Bear.
    fn table(seats: usize) -> GameSetup {
        let hands = (0..seats).map(|_| vec![creature("Spell", 1, 1)]).collect();
        let boards = (0..seats)
            .map(|seat| match seat {
                0 => vec![creature("Bear", 2, 2)],
                1 => vec![shade(), creature("Bear", 2, 2)],
                _ => Vec::new(),
            })
            .collect();
        setup(hands, boards)
    }

    fn game(setup: &GameSetup) -> GameState {
        GameState::from_setup(setup).unwrap()
    }

    fn spell_of(game: &GameState, seat: usize) -> ObjectId {
        game.zone(Zone::Hand(PlayerId::from_index(seat)))[0]
    }

    fn names(game: &GameState, targets: &[Target]) -> Vec<String> {
        targets
            .iter()
            .map(|target| match target {
                Target::Player(player) => format!("player {}", player.get()),
                Target::Object(object) => game.object(object.id).unwrap().card().name.clone(),
            })
            .collect()
    }

    #[rstest]
    #[case::own_spell_at_two_seats(2, 1, true)]
    #[case::opponents_spell_at_two_seats(2, 0, false)]
    #[case::opponents_spell_at_three_seats(3, 0, false)]
    #[case::other_opponents_spell_at_three_seats(3, 2, false)]
    #[case::own_spell_at_three_seats(3, 1, true)]
    #[case::first_opponent_at_four_seats(4, 0, false)]
    #[case::second_opponent_at_four_seats(4, 2, false)]
    #[case::third_opponent_at_four_seats(4, 3, false)]
    #[case::own_spell_at_four_seats(4, 1, true)]
    fn a_hexproof_permanent_can_be_targeted_by_its_controller_alone(
        #[case] seats: usize,
        #[case] caster: usize,
        #[case] legal: bool,
    ) {
        let game = game(&table(seats));
        let source = spell_of(&game, caster);
        let controller = PlayerId::from_index(caster);
        let shade = Target::Object(
            game.object(game.zone(Zone::Battlefield)[1])
                .unwrap()
                .reference(),
        );
        for json in [
            r#"{"what":{"Object":[{"Type":"Creature"}]}}"#,
            r#"{"what":"AnyTarget"}"#,
        ] {
            let found = legal_targets(&game, &spec(json), source, controller);
            assert_eq!(
                found.contains(&shade),
                legal,
                "{json}: {:?}",
                names(&game, &found)
            );
            // Everything else stays legal: both Bears.
            let bears = names(&game, &found).iter().filter(|n| *n == "Bear").count();
            assert_eq!(bears, 2, "{json}");
        }
    }

    #[rstest]
    #[case::own_spell_at_two_seats(2, 1, true)]
    #[case::opponents_spell_at_two_seats(2, 0, false)]
    #[case::opponents_spell_at_three_seats(3, 0, false)]
    #[case::own_spell_at_three_seats(3, 1, true)]
    #[case::opponents_spell_at_four_seats(4, 3, false)]
    #[case::own_spell_at_four_seats(4, 1, true)]
    fn a_hexproof_player_can_be_targeted_by_themselves_alone(
        #[case] seats: usize,
        #[case] caster: usize,
        #[case] legal: bool,
    ) {
        let mut setup = table(seats);
        setup.players[1].keywords = vec![Keyword::Hexproof];
        let game = game(&setup);
        let source = spell_of(&game, caster);
        let controller = PlayerId::from_index(caster);
        let holder = Target::Player(PlayerId(1));
        for json in [
            r#"{"what":"Player"}"#,
            r#"{"what":"AnyTarget"}"#,
            r#"{"what":{"Either":["Player",{"Object":[{"Type":"Land"}]}]}}"#,
        ] {
            let found = legal_targets(&game, &spec(json), source, controller);
            assert_eq!(found.contains(&holder), legal, "{json}");
            // The other players stay legal targets.
            let players = found.iter().filter(|t| t.player().is_some()).count();
            assert_eq!(players, seats - usize::from(!legal), "{json}");
        }
    }

    #[test]
    fn an_opponent_specification_never_offers_a_hexproof_opponent_and_still_offers_the_rest() {
        let mut setup = table(3);
        setup.players[1].keywords = vec![Keyword::Hexproof];
        let game = game(&setup);
        let found = legal_targets(
            &game,
            &spec(r#"{"what":"Opponent"}"#),
            spell_of(&game, 0),
            PlayerId(0),
        );
        assert_eq!(names(&game, &found), ["player 2"]);
    }

    #[rstest]
    #[case::in_a_graveyard("Graveyard")]
    #[case::in_a_hand("Hand")]
    fn a_card_with_hexproof_off_the_battlefield_can_be_targeted(#[case] zone: &str) {
        let mut setup = table(2);
        setup.players[1].battlefield.clear();
        setup.players[1].graveyard = vec![shade()];
        setup.players[1].hand = vec![shade()];
        let game = game(&setup);
        let found = legal_targets(
            &game,
            &spec(&format!(r#"{{"what":{{"Object":[{{"In":"{zone}"}}]}}}}"#)),
            spell_of(&game, 0),
            PlayerId(0),
        );
        assert!(
            names(&game, &found).contains(&"Shade".to_owned()),
            "{found:?}"
        );
    }

    #[test]
    fn the_recheck_drops_a_target_that_hexproof_protects_by_the_time_the_spell_resolves() {
        let mut setup = table(2);
        // Seat 0 controls the Shade, so its own spell may target it.
        setup.players[0].battlefield = vec![shade().into(), creature("Bear", 2, 2).into()];
        setup.players[1].battlefield.clear();
        let mut game = game(&setup);
        let source = spell_of(&game, 0);
        let reference = game.object(source).unwrap().reference();
        let shade = game.zone(Zone::Battlefield)[0];
        let bear = game.zone(Zone::Battlefield)[1];
        let shade_target = Target::Object(game.object(shade).unwrap().reference());
        let bear_target = Target::Object(game.object(bear).unwrap().reference());
        let spec = spec(r#"{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}"#);
        assert!(is_legal(&game, &spec, shade_target, source, PlayerId(0)));

        let recheck = |game: &GameState, chosen: Vec<Target>| {
            let choices = Choices {
                targets: vec![chosen],
                ..Choices::default()
            };
            let specs = [spec.clone()];
            let bindings = crate::resolve::Bindings::new();
            let scope = Scope {
                state: game,
                controller: PlayerId(0),
                source: reference,
                choices: &choices,
                specs: specs.iter().collect(),
                target_base: 0,
                departed: &[],
                bindings: &bindings,
                snapshots: Vec::new(),
                cause: None,
                layered: None,
                casting: None,
                cast_targets: None,
            };
            (
                scope.all_targets_illegal(),
                scope.refs(&Ref::Target(0)).unwrap(),
            )
        };
        assert_eq!(
            recheck(&game, vec![shade_target]),
            (false, vec![shade_target])
        );

        // Control of the Shade passes to an opponent: its hexproof now keeps the spell off it.
        game.objects.get_mut(&shade).unwrap().controller = PlayerId(1);
        assert!(!is_legal(&game, &spec, shade_target, source, PlayerId(0)));
        assert_eq!(recheck(&game, vec![shade_target]), (true, vec![]));
        // With a second, legal target the spell still resolves and the Shade is ignored.
        assert_eq!(
            recheck(&game, vec![shade_target, bear_target]),
            (false, vec![bear_target])
        );
    }

    #[test]
    fn a_player_who_gains_hexproof_before_the_spell_resolves_is_no_longer_a_legal_target() {
        let mut game = game(&table(2));
        let source = spell_of(&game, 0);
        let any_player = spec(r#"{"what":"Player"}"#);
        let target = Target::Player(PlayerId(1));
        assert!(is_legal(&game, &any_player, target, source, PlayerId(0)));
        game.players[1].keywords.push(Keyword::Hexproof);
        assert!(!is_legal(&game, &any_player, target, source, PlayerId(0)));
        assert!(is_legal(&game, &any_player, target, source, PlayerId(1)));
    }
}
