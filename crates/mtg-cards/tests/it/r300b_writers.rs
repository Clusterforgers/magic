//! Goal: the r30-0b writers word exactly the shapes they stand for (an upkeep return counted by the cards above, a hand shuffled in and drawn again for you or each player, control passed to a player who is not the owner, a mana rider about the commander's casts, the colors among permanents, an ability only the owner activates and damage redirected to the owner, the owner losing half their life, a token copy that is a red Balloon with keywords until the next end step, a draw after excess damage, a search that makes the card a copy, empower Jace) and nothing near them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::written_variants;
use rstest::rstest;

fn says(json: &str, text: &str) -> bool {
    let ability: Ability = serde_json::from_str(json).unwrap();
    written_variants(&ability)
        .iter()
        .any(|written| written == text)
}

#[rstest]
#[case::upkeep_above(
    r#"{"Activated":{"cost":"{B}","condition":{"All":["YourTurn",{"Step":"Upkeep"},{"Compare":{"left":{"CardsAbove":{"filter":[{"Type":"Creature"}]}},"op":"Ge","right":3}}]},"from":"Graveyard","effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#,
    "{B}: Return ~ from your graveyard to the battlefield. Activate only during your upkeep and only if three or more creature cards are above ~."
)]
#[case::each_player_shuffles(
    r#"{"Activated":{"cost":[{"Mana":"{R}"},"SacrificeSelf"],"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Let":{"as":"n","amount":{"Count":[{"In":"Hand"},{"OwnedBy":{"Bound":"player"}}]}}},{"Move":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Bound":"player"}}]},"to":{"Library":{"place":"Shuffled"}}}},{"Draw":{"who":{"Bound":"player"},"count":{"Bound":"n"}}}]}}]}}"#,
    "{R}, Sacrifice ~: Each player shuffles the cards from their hand into their library, then draws that many cards."
)]
#[case::own_hand_shuffled(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Let":{"as":"n","amount":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}},{"Move":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]},"to":{"Library":{"place":"Shuffled"}}}},{"Draw":{"count":{"Bound":"n"}}}]}}"#,
    "When ~ enters, shuffle the cards from your hand into your library, then draw that many cards."
)]
#[case::owner_passes(
    r#"{"Activated":{"cost":"{2}","condition":"YourTurn","targets":[{"what":{"PlayerAmong":{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"Matches":{"subject":"This","selector":[{"OwnedBy":"Asker"}]}}}}}}}],"effects":[{"Continuous":{"affected":"This","mods":[{"SetController":{"Target":0}}],"duration":"Forever"}}]}}"#,
    "{2}: Target player other than ~'s owner gains control of it. Activate only during your turn."
)]
#[case::scry_commander_casts(
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"when_spent":{"spell":{"facts":["YourCommander"]},"effects":[{"Scry":{"count":{"CastsFromCommand":"Triggering"}}}]}}}]}}"#,
    "{1}, {T}: Add one mana of any color. When you spend this mana to cast your commander, scry X, where X is the number of times it's been cast from the command zone this game."
)]
#[case::enters_commander_casts(
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"CommanderIdentity":1},"only":[{"Enters":{"spell":{"facts":["YourCommander"]},"per_command_cast":true}}]}}]}}"#,
    "{1}, {T}: Add one mana of any color in your commander's color identity. If you spend this mana to cast your commander, it enters with a number of additional +1/+1 counters on it equal to the number of times it's been cast from the command zone this game."
)]
#[case::colors_among(
    r#"{"Activated":{"cost":[{"Mana":"{5}"},"Tap"],"effects":[{"Draw":{"count":{"Distinct":{"of":[{"ControlledBy":"You"}],"stat":"Colors"}}}}]}}"#,
    "{5}, {T}: Draw a card for each color among permanents you control."
)]
#[case::owner_only_redirect(
    r#"{"Activated":{"cost":"{0}","by":"Owner","effects":[{"Shield":{"when":{"Damage":{"to":"This"}},"by":{"RedirectUpTo":{"most":1,"to":{"OwnerOf":"This"}}},"duration":"UntilEndOfTurn"}}]}}"#,
    "{0}: The next 1 damage that would be dealt to ~ this turn is dealt to its owner instead. Only ~'s owner may activate this ability."
)]
#[case::owner_loses_half(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"LoseLife":{"who":{"OwnerOf":"This"},"amount":{"Half":{"of":{"Life":{"OwnerOf":"This"}},"up":true}}}}]}}"#,
    "When ~ dies, its owner loses half their life, rounded up."
)]
#[case::balloon_copy(
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"timing":"Sorcery","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"SetPT":{"power":1,"toughness":1}},{"AddColors":["Red"]},{"AddSubtype":"Balloon"},{"AddType":"Creature"},{"AddKeyword":"Flying"},{"AddKeyword":"Haste"}]}},"as":"copy"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Sacrifice":{"filter":[{"Is":{"Bound":"copy"}}]}}]}}]}}"#,
    "{1}, {T}: Create a token that's a copy of another target creature you control, except it's a 1/1 red Balloon creature in addition to its other colors and types and it has flying and haste. Sacrifice it at the beginning of the next end step. Activate only as a sorcery."
)]
#[case::excess_draws(
    r#"{"Activated":{"cost":[{"Mana":"{4}{R}"},"Untap",{"Discard":{}}],"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":"This","stat":"Power"}},"excess":{"as":"excess"}}},{"If":{"condition":{"All":[{"Compare":{"left":{"Bound":"excess"},"op":"Ge","right":1}},{"Matches":{"subject":{"Target":0},"selector":[{"Type":"Creature"}]}}]},"then":[{"Draw":{}}]}}]}}"#,
    "{4}{R}, {Q}, Discard a card: ~ deals damage equal to its power to any target. If excess damage was dealt to a creature this way, draw a card."
)]
#[case::search_become_copy(
    r#"{"Activated":{"cost":"{2}","effects":[{"Search":{"filter":[{"Subtype":"Dragon"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"},{"Type":"Battle"}]}],"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":"Graveyard"}},{"If":{"condition":{"CountOf":{"subject":{"Bound":"found"},"selector":[{"In":"Graveyard"}],"op":"Ge","right":1}},"then":[{"BecomeCopy":{"what":"This","of":{"Bound":"found"},"duration":"UntilEndOfTurn"}}]}},{"Shuffle":{}}]}}"#,
    "{2}: Search your library for a Dragon permanent card and put it into your graveyard. If you do, ~ becomes a copy of that card until end of turn. Then shuffle."
)]
#[case::empower(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Jace"},"Token",{"ControlledBy":"You"}]},"op":"Lt","right":1}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Jace","colors":["Blue"],"types":["Planeswalker"],"subtypes":["Jace"],"abilities":[{"Loyalty":{"cost":{"Minus":1},"effects":[{"Surveil":{}}]}},{"Loyalty":{"cost":{"Minus":3},"effects":[{"Draw":{}}]}}]}}}}]}},{"Choose":{"from":{"Each":[{"Subtype":"Jace"},"Token",{"ControlledBy":"You"}]},"as":"jace"}},{"PutCounters":{"on":{"Bound":"jace"},"counter":"loyalty","count":3}}]}}"#,
    "When ~ enters, empower Jace 3."
)]
fn the_shape_is_worded_as_the_card_prints_it(#[case] json: &str, #[case] text: &str) {
    assert!(says(json, text), "{text}");
}

#[rstest]
#[case::upkeep_above_at_most(
    r#"{"Activated":{"cost":"{B}","condition":{"All":["YourTurn",{"Step":"Upkeep"},{"Compare":{"left":{"CardsAbove":{"filter":[{"Type":"Creature"}]}},"op":"Le","right":3}}]},"from":"Graveyard","effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#,
    "{B}: Return ~ from your graveyard to the battlefield. Activate only during your upkeep and only if three or more creature cards are above ~."
)]
#[case::upkeep_above_directly(
    r#"{"Activated":{"cost":"{B}","condition":{"All":["YourTurn",{"Step":"Upkeep"},{"Compare":{"left":{"CardsAbove":{"filter":[{"Type":"Creature"}],"directly":true}},"op":"Ge","right":3}}]},"from":"Graveyard","effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#,
    "{B}: Return ~ from your graveyard to the battlefield. Activate only during your upkeep and only if three or more creature cards are above ~."
)]
#[case::each_player_only_you(
    r#"{"Activated":{"cost":[{"Mana":"{R}"},"SacrificeSelf"],"effects":[{"RepeatEach":{"over":"Opponents","as":"player","effects":[{"Let":{"as":"n","amount":{"Count":[{"In":"Hand"},{"OwnedBy":{"Bound":"player"}}]}}},{"Move":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Bound":"player"}}]},"to":{"Library":{"place":"Shuffled"}}}},{"Draw":{"who":{"Bound":"player"},"count":{"Bound":"n"}}}]}}]}}"#,
    "{R}, Sacrifice ~: Each player shuffles the cards from their hand into their library, then draws that many cards."
)]
#[case::own_hand_draws_one(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Let":{"as":"n","amount":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}},{"Move":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]},"to":{"Library":{"place":"Shuffled"}}}},{"Draw":{}}]}}"#,
    "When ~ enters, shuffle the cards from your hand into your library, then draw that many cards."
)]
#[case::owner_passes_until_eot(
    r#"{"Activated":{"cost":"{2}","condition":"YourTurn","targets":[{"what":{"PlayerAmong":{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"Matches":{"subject":"This","selector":[{"OwnedBy":"Asker"}]}}}}}}}],"effects":[{"Continuous":{"affected":"This","mods":[{"SetController":{"Target":0}}],"duration":"UntilEndOfTurn"}}]}}"#,
    "{2}: Target player other than ~'s owner gains control of it. Activate only during your turn."
)]
#[case::scry_all_commanders(
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"when_spent":{"spell":{"facts":["YourCommander"]},"effects":[{"Scry":{"count":{"CommanderCasts":"You"}}}]}}}]}}"#,
    "{1}, {T}: Add one mana of any color. When you spend this mana to cast your commander, scry X, where X is the number of times it's been cast from the command zone this game."
)]
#[case::enters_creature_spell(
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"CommanderIdentity":1},"only":[{"Enters":{"spell":{"types":["Creature"]},"per_command_cast":true}}]}}]}}"#,
    "{1}, {T}: Add one mana of any color in your commander's color identity. If you spend this mana to cast your commander, it enters with a number of additional +1/+1 counters on it equal to the number of times it's been cast from the command zone this game."
)]
#[case::colors_of_creatures(
    r#"{"Activated":{"cost":[{"Mana":"{5}"},"Tap"],"effects":[{"Draw":{"count":{"Distinct":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Colors"}}}}]}}"#,
    "{5}, {T}: Draw a card for each color among permanents you control."
)]
#[case::redirect_to_you(
    r#"{"Activated":{"cost":"{0}","by":"Owner","effects":[{"Shield":{"when":{"Damage":{"to":"This"}},"by":{"RedirectUpTo":{"most":1,"to":"You"}},"duration":"UntilEndOfTurn"}}]}}"#,
    "{0}: The next 1 damage that would be dealt to ~ this turn is dealt to its owner instead. Only ~'s owner may activate this ability."
)]
#[case::controller_activates(
    r#"{"Activated":{"cost":"{0}","effects":[{"Shield":{"when":{"Damage":{"to":"This"}},"by":{"RedirectUpTo":{"most":1,"to":{"OwnerOf":"This"}}},"duration":"UntilEndOfTurn"}}]}}"#,
    "{0}: The next 1 damage that would be dealt to ~ this turn is dealt to its owner instead. Only ~'s owner may activate this ability."
)]
#[case::half_rounded_down(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"LoseLife":{"who":{"OwnerOf":"This"},"amount":{"Half":{"of":{"Life":{"OwnerOf":"This"}},"up":false}}}}]}}"#,
    "When ~ dies, its owner loses half their life, rounded up."
)]
#[case::balloon_one_keyword(
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"timing":"Sorcery","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"SetPT":{"power":1,"toughness":1}},{"AddColors":["Red"]},{"AddSubtype":"Balloon"},{"AddType":"Creature"},{"AddKeyword":"Flying"}]}},"as":"copy"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Sacrifice":{"filter":[{"Is":{"Bound":"copy"}}]}}]}}]}}"#,
    "{1}, {T}: Create a token that's a copy of another target creature you control, except it's a 1/1 red Balloon creature in addition to its other colors and types and it has flying and haste. Sacrifice it at the beginning of the next end step. Activate only as a sorcery."
)]
#[case::balloon_kept(
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"timing":"Sorcery","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"SetPT":{"power":1,"toughness":1}},{"AddColors":["Red"]},{"AddSubtype":"Balloon"},{"AddType":"Creature"},{"AddKeyword":"Flying"},{"AddKeyword":"Haste"}]}},"as":"copy"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"Upkeep"}},"effects":[{"Sacrifice":{"filter":[{"Is":{"Bound":"copy"}}]}}]}}]}}"#,
    "{1}, {T}: Create a token that's a copy of another target creature you control, except it's a 1/1 red Balloon creature in addition to its other colors and types and it has flying and haste. Sacrifice it at the beginning of the next end step. Activate only as a sorcery."
)]
#[case::excess_to_any(
    r#"{"Activated":{"cost":[{"Mana":"{4}{R}"},"Untap",{"Discard":{}}],"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":"This","stat":"Power"}},"excess":{"as":"excess"}}},{"If":{"condition":{"All":[{"Compare":{"left":{"Bound":"excess"},"op":"Ge","right":1}},{"Matches":{"subject":{"Target":0},"selector":[{"Type":"Planeswalker"}]}}]},"then":[{"Draw":{}}]}}]}}"#,
    "{4}{R}, {Q}, Discard a card: ~ deals damage equal to its power to any target. If excess damage was dealt to a creature this way, draw a card."
)]
#[case::copy_forever(
    r#"{"Activated":{"cost":"{2}","effects":[{"Search":{"filter":[{"Subtype":"Dragon"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"},{"Type":"Battle"}]}],"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":"Graveyard"}},{"If":{"condition":{"CountOf":{"subject":{"Bound":"found"},"selector":[{"In":"Graveyard"}],"op":"Ge","right":1}},"then":[{"BecomeCopy":{"what":"This","of":{"Bound":"found"},"duration":"Forever"}}]}},{"Shuffle":{}}]}}"#,
    "{2}: Search your library for a Dragon permanent card and put it into your graveyard. If you do, ~ becomes a copy of that card until end of turn. Then shuffle."
)]
#[case::empower_two(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Jace"},"Token",{"ControlledBy":"You"}]},"op":"Lt","right":1}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Jace","colors":["Blue"],"types":["Planeswalker"],"subtypes":["Jace"],"abilities":[{"Loyalty":{"cost":{"Minus":1},"effects":[{"Surveil":{}}]}},{"Loyalty":{"cost":{"Minus":3},"effects":[{"Draw":{}}]}}]}}}}]}},{"Choose":{"from":{"Each":[{"Subtype":"Jace"},"Token",{"ControlledBy":"You"}]},"as":"jace"}},{"PutCounters":{"on":{"Bound":"jace"},"counter":"loyalty","count":2}}]}}"#,
    "When ~ enters, empower Jace 3."
)]
fn a_near_miss_is_not_worded_as_the_card(#[case] json: &str, #[case] text: &str) {
    assert!(!says(json, text), "{text}");
}
