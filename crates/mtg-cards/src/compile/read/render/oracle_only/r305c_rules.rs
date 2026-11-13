//! Wordings of two static rules: "You may play lands and cast spells from the top of your library." and "Your opponents
//! can't cast spells during combat."

use crate::ability::Ability;
use crate::modify::{CastPermission, GameRule, Prohibited, SpendAs};
use crate::refs::{Ref, Selector, ZoneKind};
use crate::value::Condition;

/// Adds the wording of a permission to play lands and cast spells from the top of the library, or of a ban on casting
/// spells during combat.
pub(super) fn r305c_add_rules(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule,
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return;
    };
    match rule {
        GameRule::MayCast(permission) if r305c_plays_from_the_top(permission) => {
            texts.push(
                "You may play lands and cast spells from the top of your library.".to_owned(),
            );
        }
        GameRule::Cant {
            who,
            action:
                Prohibited::Cast {
                    filter,
                    more_than: None,
                    unless: Some(Condition::Not(during)),
                },
        } if filter.is_empty() && **during == Condition::InCombat => {
            let players = match who {
                Ref::Opponents => "Your opponents",
                Ref::EachPlayer => "Players",
                _ => return,
            };
            texts.push(format!("{players} can't cast spells during combat."));
        }
        _ => {}
    }
}

/// Whether the permission is "play lands and cast spells from the top of your library" and nothing more.
fn r305c_plays_from_the_top(permission: &CastPermission) -> bool {
    *permission
        == CastPermission {
            who: Ref::You,
            what: Selector::default(),
            from: Some(ZoneKind::Library),
            free: false,
            flash: false,
            only: None,
            play: true,
            once: false,
            additional: None,
            rather: None,
            spend_as: SpendAs::Normal,
            when: None,
            any_cast: false,
            riders: Vec::new(),
            limit: None,
        }
}

#[cfg(test)]
mod tests {
    //! Goal: the two rules read as printed, and near misses (a filter, another zone, a ban outside combat) are not.
    use super::*;
    use rstest::rstest;

    fn written(json: &str) -> Vec<String> {
        let ability: Ability = serde_json::from_str(json).unwrap();
        let mut texts = Vec::new();
        r305c_add_rules(&ability, &mut texts);
        texts
    }

    #[rstest]
    #[case::frenzy(
        r#"{"Rule":{"rule":{"MayCast":{"from":"Library","play":true}}}}"#,
        Some("You may play lands and cast spells from the top of your library.")
    )]
    #[case::frenzy_with_filter(r#"{"Rule":{"rule":{"MayCast":{"what":[{"Type":"Creature"}],"from":"Library","play":true}}}}"#, None)]
    #[case::frenzy_from_graveyard(
        r#"{"Rule":{"rule":{"MayCast":{"from":"Graveyard","play":true}}}}"#,
        None
    )]
    #[case::megatron(r#"{"Rule":{"rule":{"Cant":{"who":"Opponents","action":{"Cast":{"unless":{"Not":"InCombat"}}}}}}}"#, Some("Your opponents can't cast spells during combat."))]
    #[case::outside_combat(
        r#"{"Rule":{"rule":{"Cant":{"who":"Opponents","action":{"Cast":{"unless":"InCombat"}}}}}}"#,
        None
    )]
    #[case::creature_spells(r#"{"Rule":{"rule":{"Cant":{"who":"Opponents","action":{"Cast":{"filter":[{"Type":"Creature"}],"unless":{"Not":"InCombat"}}}}}}}"#, None)]
    fn r305c_rules_read_back(#[case] json: &str, #[case] text: Option<&str>) {
        assert_eq!(written(json).first().map(String::as_str), text);
    }
}
