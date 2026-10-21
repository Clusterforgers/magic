//! `Effect::Attach`: Auras, Equipment and Fortifications are attached to an object (CR 701.3).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::attach::legal_host;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent `what` names that is still the object named and may be attached to the first permanent `to` names moves
/// onto it, all together, in the order the reference lists them. One that may not be attached there stays where it is
/// (CR 701.3b), one already attached there is not attached again (CR 701.3c), and naming no permanent to attach to attaches
/// nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Attach { what, to } = effect else {
        return Err(wrong_effect("Attach"));
    };
    let state = cx.state();
    let Some(host) = cx
        .scope
        .live_objects(to)?
        .into_iter()
        .find(|object| object.zone() == Zone::Battlefield)
    else {
        return Ok(Run::events(Vec::new()));
    };
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            state.host_of(object.id()) != Some(host.id()) && legal_host(state, object.id(), host.id())
        })
        .map(|object| Event::Attached {
            object: object.id(),
            to: host.id(),
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: an attach moves each attachment it names onto the permanent it names when that permanent may hold it (an Aura what its enchant ability allows, an Equipment a creature, a Fortification a land), and leaves alone one that may not, one already attached there, an object that has left and a name that repeats; it changes nothing but the attachments, binds nothing, and refuses another effect by name.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_core::card::{CardType, Subtype};
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::setup::PresetPermanent;
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn card(name: &str, types: &[CardType], subtype: &str, rules: &[&str]) -> Arc<CardDefinition> {
        let mut card = CardDefinition::new(name);
        for ty in types {
            card = card.with_type(*ty);
        }
        if !subtype.is_empty() {
            card.subtypes.push(Subtype(subtype.into()));
        }
        for json in rules {
            let ability: Ability = serde_json::from_str(json).unwrap();
            card = card.with_rules(ability);
        }
        Arc::new(card)
    }

    const ENCHANT_CREATURE: &str = r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}}"#;
    const ENCHANT_LAND: &str = r#"{"Keyword":{"Enchant":[{"Type":"Land"}]}}"#;

    /// Seat 0 has, in this order: a Bear (0), a Forest (1), a Pacifism-like Aura on creatures that starts on the Bear (2),
    /// a Blade Equipment on nothing (3), a Wall-Crusher Fortification on nothing (4), a Rune Aura on lands on nothing (5),
    /// and a Wolf (6).
    fn game() -> GameState {
        let mut setup = crate::fixtures::setup(
            vec![vec![], vec![]],
            vec![
                vec![
                    card("Bear", &[CardType::Creature], "", &[]),
                    card("Forest", &[CardType::Land], "", &[]),
                    card("Halo", &[CardType::Enchantment], "Aura", &[ENCHANT_CREATURE]),
                    card("Blade", &[CardType::Artifact], "Equipment", &[]),
                    card("Crusher", &[CardType::Artifact], "Fortification", &[]),
                    card("Rune", &[CardType::Enchantment], "Aura", &[ENCHANT_LAND]),
                    card("Wolf", &[CardType::Creature], "", &[]),
                ],
                vec![],
            ],
        );
        setup.players[0].battlefield[2] = PresetPermanent {
            attached_to: Some((0, 0)),
            ..setup.players[0].battlefield[2].clone()
        };
        GameState::from_setup(&setup).unwrap()
    }

    fn id(game: &GameState, name: &str) -> ObjectId {
        game.objects().find(|o| o.card().name == name).unwrap().id()
    }

    /// Runs `effect` with `Bound("a")` standing for `attachments` and `Bound("h")` for `host`.
    fn attach(game: &GameState, effect: &Effect) -> Result<Run, ResolveError> {
        let source = game.object(id(game, "Bear")).unwrap();
        let mut resolving = Resolving::begin(source.id());
        for (name, names) in [("a", &["Halo", "Blade"][..]), ("h", &["Wolf"][..])] {
            resolving.bindings.insert(
                name.into(),
                names
                    .iter()
                    .map(|n| crate::choose::Target::Object(game.object(id(game, n)).unwrap().reference()))
                    .collect(),
            );
        }
        let mut scope = Scope::announcing(game, ME, source.reference());
        scope.bindings = &resolving.bindings;
        let cx = Cx {
            scope,
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    fn effect(what: &str, to: &str) -> Effect {
        serde_json::from_str(&format!(r#"{{"Attach":{{"what":{what},"to":{to}}}}}"#)).unwrap()
    }

    fn attached(run: Run, game: &GameState) -> Vec<(String, String)> {
        let Run::Done { events, next, .. } = run else {
            panic!("an attach asks nothing");
        };
        assert_eq!(next, Next::Sibling);
        events
            .iter()
            .map(|event| {
                let Event::Attached { object, to } = event else {
                    panic!("only attachments: {event:?}");
                };
                (
                    game.object(*object).unwrap().card().name.clone(),
                    game.object(*to).unwrap().card().name.clone(),
                )
            })
            .collect()
    }

    fn names(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    }

    #[rstest]
    #[case::an_aura_moves_to_a_creature_it_may_enchant(r#"{"Bound":"a"}"#, r#"{"Bound":"h"}"#, &[("Blade", "Wolf"), ("Halo", "Wolf")])]
    #[case::an_equipment_moves_to_a_creature(r#"{"Each":[{"Subtype":"Equipment"}]}"#, r#"{"Bound":"h"}"#, &[("Blade", "Wolf")])]
    #[case::an_aura_does_not_move_to_what_it_may_not_enchant(r#"{"Each":[{"Subtype":"Aura"}]}"#, r#"{"Each":[{"Type":"Land"}]}"#, &[("Rune", "Forest")])]
    #[case::a_fortification_moves_to_a_land(r#"{"Each":[{"Subtype":"Fortification"}]}"#, r#"{"Each":[{"Type":"Land"}]}"#, &[("Crusher", "Forest")])]
    #[case::a_fortification_does_not_move_to_a_creature(r#"{"Each":[{"Subtype":"Fortification"}]}"#, r#"{"Bound":"h"}"#, &[])]
    #[case::an_equipment_does_not_move_to_a_land(r#"{"Each":[{"Subtype":"Equipment"}]}"#, r#"{"Each":[{"Type":"Land"}]}"#, &[])]
    #[case::nothing_is_attached_to_itself(r#"{"Each":[{"Subtype":"Equipment"}]}"#, r#"{"Each":[{"Subtype":"Equipment"}]}"#, &[])]
    #[case::naming_nothing_attaches_nothing(r#"{"Each":[{"Type":"Planeswalker"}]}"#, r#"{"Bound":"h"}"#, &[])]
    fn an_attach_moves_what_the_host_may_hold(
        #[case] what: &str,
        #[case] to: &str,
        #[case] moved: &[(&str, &str)],
    ) {
        let game = game();
        let run = attach(&game, &effect(what, to)).unwrap();
        let mut got = attached(run, &game);
        got.sort();
        let mut expected = names(moved);
        expected.sort();
        assert_eq!(got, expected);
    }

    #[test]
    fn an_aura_already_on_the_host_is_not_attached_again() {
        let game = game();
        let run = attach(&game, &effect(r#"{"Each":[{"Subtype":"Aura"},{"Type":"Enchantment"}]}"#, r#"{"Each":[{"Named":"Bear"}]}"#)).unwrap();
        // Halo starts on the Bear; Rune may not enchant a creature.
        assert_eq!(attached(run, &game), []);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let game = game();
        let error = attach(&game, &serde_json::from_str(r#"{"Draw":{}}"#).unwrap()).unwrap_err();
        assert_eq!(
            error,
            ResolveError::Invalid("the Attach primitive was given another effect".into())
        );
    }
}
