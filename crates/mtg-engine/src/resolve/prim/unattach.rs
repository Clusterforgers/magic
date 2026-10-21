//! `Effect::Unattach`: Auras, Equipment and Fortifications come off what they are attached to (CR 701.3d).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// Each permanent `what` names that is still the object named and is attached to something becomes unattached, all
/// together, in the order the reference lists them; one attached to nothing stays as it is.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Unattach { what } = effect else {
        return Err(wrong_effect("Unattach"));
    };
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            object
                .permanent()
                .is_some_and(|permanent| permanent.attached_to().is_some())
        })
        .map(|object| Event::Unattached {
            object: object.id(),
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: an unattach takes each attachment it names off its host, leaves alone one that is attached to nothing, an object that has left and a name that repeats, changes nothing but the attachments, and refuses another effect by name.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::{CardType, Subtype};
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::setup::PresetPermanent;
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn card(name: &str, types: &[CardType], subtype: &str) -> Arc<CardDefinition> {
        let mut card = CardDefinition::new(name);
        for ty in types {
            card = card.with_type(*ty);
        }
        if !subtype.is_empty() {
            card.subtypes.push(Subtype(subtype.into()));
        }
        Arc::new(card)
    }

    /// Seat 0 has, in this order: a Bear (0), a Blade Equipment on the Bear (1), a Shield Equipment on nothing (2) and a
    /// Halo Aura on the Bear (3).
    fn game() -> GameState {
        let mut setup = crate::fixtures::setup(
            vec![vec![], vec![]],
            vec![
                vec![
                    card("Bear", &[CardType::Creature], ""),
                    card("Blade", &[CardType::Artifact], "Equipment"),
                    card("Shield", &[CardType::Artifact], "Equipment"),
                    card("Halo", &[CardType::Enchantment], "Aura"),
                ],
                vec![],
            ],
        );
        for place in [1, 3] {
            setup.players[0].battlefield[place] = PresetPermanent {
                attached_to: Some((0, 0)),
                ..setup.players[0].battlefield[place].clone()
            };
        }
        GameState::from_setup(&setup).unwrap()
    }

    fn unattached(what: &str) -> Vec<String> {
        let game = game();
        let source = game.zone(crate::zones::Zone::Battlefield)[0];
        let source = game.object(source).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect =
            serde_json::from_str(&format!(r#"{{"Unattach":{{"what":{what}}}}}"#)).unwrap();
        let Run::Done { events, next, .. } = run(&cx, &effect).unwrap() else {
            panic!("an unattach asks nothing");
        };
        assert_eq!(next, Next::Sibling);
        let mut names: Vec<String> = events
            .iter()
            .map(|event| {
                let Event::Unattached { object } = event else {
                    panic!("only unattachments: {event:?}");
                };
                game.object(*object).unwrap().card().name.clone()
            })
            .collect();
        names.sort();
        names
    }

    #[rstest]
    #[case::an_equipment(r#"{"Each":[{"Named":"Blade"}]}"#, &["Blade"])]
    #[case::an_aura(r#"{"Each":[{"Subtype":"Aura"}]}"#, &["Halo"])]
    #[case::everything_attached(r#"{"Each":[{"Not":{"Type":"Creature"}}]}"#, &["Blade", "Halo"])]
    #[case::one_attached_to_nothing_stays(r#"{"Each":[{"Named":"Shield"}]}"#, &[])]
    #[case::the_source_is_not_attached(r#""This""#, &[])]
    #[case::nothing_named(r#"{"Each":[{"Named":"Nobody"}]}"#, &[])]
    fn an_unattach_takes_what_it_names_off_its_host(#[case] what: &str, #[case] taken: &[&str]) {
        assert_eq!(unattached(what), taken);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let game = game();
        let source = game
            .object(game.zone(crate::zones::Zone::Battlefield)[0])
            .unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let error = run(&cx, &serde_json::from_str(r#"{"Draw":{}}"#).unwrap()).unwrap_err();
        assert_eq!(
            error,
            ResolveError::Invalid("the Unattach primitive was given another effect".into())
        );
    }
}
