//! The wording of effects that happen as a permanent enters (`ReplaceBy::AsEnters`): "As ~ enters, discard your hand".

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// "As ~ enters, <the effects as a spell writes them>", without the closing period; empty when no writer reads them.
pub(super) fn texts(effects: &[Effect]) -> Vec<String> {
    let spell = Ability::Spell {
        targets: Vec::new(),
        effects: effects.to_vec(),
    };
    // The same steps as an enters trigger writes them ("When ~ enters, sacrifice all lands you control").
    let trigger = Ability::Triggered {
        when: TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: None,
            to: Some(ZoneKind::Battlefield),
        },
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: effects.to_vec(),
    };
    let triggered = super::render::written_variants(&trigger)
        .into_iter()
        .filter_map(|text| text.strip_prefix("When ~ enters, ").map(str::to_owned));
    let mut written: Vec<String> = Vec::new();
    for text in super::render::written_variants(&spell)
        .into_iter()
        .chain(triggered)
    {
        if !written.contains(&text) {
            written.push(text);
        }
    }
    written
        .iter()
        .filter_map(|text| {
            let body = text.strip_suffix('.')?;
            let mut chars = body.chars();
            let first = chars.next()?;
            Some(format!(
                "As ~ enters, {}{}",
                first.to_lowercase(),
                chars.as_str()
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: effects as a permanent enters read back as the spell's or the enters trigger's words after "As ~ enters,"
    //! and a near miss (another amount, another kind) is not written as the printed clause.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::lose_life(
        r#"[{"LoseLife":{"amount":{"Life":"You"}}}]"#,
        "As ~ enters, you lose life equal to your life total",
        true
    )]
    #[case::lose_two(
        r#"[{"LoseLife":{"amount":2}}]"#,
        "As ~ enters, you lose life equal to your life total",
        false
    )]
    #[case::discard_hand(
        r#"[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}]"#,
        "As ~ enters, discard your hand",
        true
    )]
    #[case::discard_one(r#"[{"Discard":{}}]"#, "As ~ enters, discard your hand", false)]
    #[case::sacrifice_lands(r#"[{"Sacrifice":{"filter":[{"Type":"Land"},{"ControlledBy":"You"}],"count":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}]"#, "As ~ enters, sacrifice all lands you control", true)]
    #[case::sacrifice_two_lands(
        r#"[{"Sacrifice":{"filter":[{"Type":"Land"},{"ControlledBy":"You"}],"count":2}}]"#,
        "As ~ enters, sacrifice all lands you control",
        false
    )]
    #[case::sacrifice_creatures(r#"[{"Sacrifice":{"filter":[{"Type":"Creature"},{"ControlledBy":"You"}],"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}]"#, "As ~ enters, sacrifice all lands you control", false)]
    #[case::face_down(
        r#"[{"TurnFaceDown":{"what":{"Each":[{"Type":"Creature"},"Other",{"Not":"Token"}]}}}]"#,
        "As ~ enters, turn all other nontoken creatures face down",
        true
    )]
    fn the_effects_read_after_as_it_enters(
        #[case] json: &str,
        #[case] printed: &str,
        #[case] written: bool,
    ) {
        let effects: Vec<Effect> = serde_json::from_str(json).expect("effects");
        let texts = texts(&effects);
        assert_eq!(texts.iter().any(|t| t == printed), written, "{texts:?}");
    }
}
