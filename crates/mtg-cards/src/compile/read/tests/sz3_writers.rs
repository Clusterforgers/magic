//! Goal: the writers of the third sample of S to Z give the oracle wording of a sacrifice that another avoids, spells cast with flash, a battalion, a lower-case duration, a card that returns itself, three colors in a list, half of X tokens, a group that excludes a supertype, a counter more of each kind, the destruction of the creature that dealt damage, a card put into your graveyard, damage dealt to you, "at least" other creatures, a comma inside a quote, an entry and a step, a free cast from the hand, a doubling after a choice of targets, a prevention that costs a counter, counters from two counts, spells that target something, the card of a creature that died, a kicked entry, the spells you cast as X, an attach, a spell cast from the hand, tokens as many as the targets, two returned cards, a trigger again, a rule under a condition, damage both ways, damage to each of several kinds, an exile from the battlefield, a choice of keywords and a permanent you own, and a wrong player, count, kind, zone, condition or duration never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const GITROG: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Pay":{"cost":[{"Sacrifice":{"filter":[{"Type":"Land"}]}}],"unpaid":[{"Sacrifice":{"filter":["This"]}}]}}]}}"#;
const YEVA: &str = r#"{"Rule":{"rule":{"MayCast":{"what":[{"Color":"Green"},{"Type":"Creature"}],"flash":true}}}}"#;
const SIGARDA: &str = r#"{"Rule":{"rule":{"MayCast":{"what":[{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"}]}],"flash":true}}}}"#;
const TAJIC: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at_least":3}},"if":{"Matches":{"subject":"This","selector":["Attacking"]}},"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":5,"toughness":5}}],"duration":"UntilEndOfTurn"}}]}}"#;
const CICADA: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":["Colorless"],"by":"You"}},"effects":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Trample"},{"ModifyPT":{"power":{"Stat":{"of":"Triggering","stat":"ManaValue"}},"toughness":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}],"duration":"UntilEndOfTurn"}}]}}"#;
const SPIT_FLAME: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Subtype":"Dragon"},{"ControlledBy":"You"}],"to":"Battlefield"}},"from":"Graveyard","effects":[{"Pay":{"cost":"{R}","paid":[{"Move":{"what":"This","to":"Hand"}}]}}]}}"#;
const SAND_SCOUT: &str = r#"{"Triggered":{"when":{"Batch":{"ZoneChange":{"what":[{"Type":"Land"},{"OwnedBy":"You"}],"to":"Graveyard"}}},"limit":"OncePerTurn","effects":[{"CreateToken":{"token":{"Custom":{"name":"Sand Warrior","colors":["Red","Green","White"],"types":["Creature"],"subtypes":["Sand","Warrior"],"power":1,"toughness":1}}}}]}}"#;
const GOOSE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"CreateToken":{"token":{"Named":"food"},"count":{"Half":{"of":"X","up":true}}}}]}}"#;
const BLAST: &str = r#"{"Spell":{"effects":[{"Exile":{"what":{"Each":[{"Not":{"Type":"Land"}},{"Not":{"Supertype":"Legendary"}}]}}}]}}"#;
const CONSTRICTOR: &str = r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]},{"ControlledBy":"You"}]}}},"by":{"Add":1}}}"#;
const CONSTRICTOR_YOU: &str =
    r#"{"Replacement":{"when":{"CountersPut":{"on":"You"}},"by":{"Add":1}}}"#;
const TEYSA: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Type":"Creature"}],"to":"You","combat":true}},"effects":[{"Destroy":{"what":"Dealer"}}]}}"#;
const SKOLA: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"OwnedBy":"You"}],"to":"Graveyard"}},"effects":[{"GainLife":{"amount":1}}]}}"#;
const DROPLET: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[],"to":"You"}},"effects":[{"PutCounters":{"on":"This","counter":"charge","count":"Triggering"}}]}}"#;
const SVYELUN: &str = r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Indestructible"}],"condition":{"Compare":{"left":{"Count":[{"Subtype":"Merfolk"},"Other",{"ControlledBy":"You"}]},"op":"Ge","right":2}}}}"#;
const NOULITHS: &str = r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":0}},{"GrantAbility":{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking"]}}],"effects":[{"Untap":{"what":{"Target":0}}}]}}},{"AddSubtype":"Cleric"}]}}"#;
const TAIL: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"StepBegins":{"step":"Upkeep","whose":"You"}}]},"effects":[{"Surveil":{}}]}}"#;
const SURTLAND: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]},"count":{"min":0,"max":1},"as":"spell"}},{"CastFromZone":{"what":{"Bound":"spell"},"free":true}}]}}"#;
const SOLIDARITY: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":null}}],"effects":[{"MultiplyCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#;
const HORDE_DAMAGE: &str =
    r#"{"Replacement":{"when":{"Damage":{"to":"This"}},"by":{"PreventRemovingCounter":"+1/+1"}}}"#;
const HORDE_ENTERS: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":{"Sum":[{"Count":[{"Subtype":"Zombie"},"Other",{"ControlledBy":"You"}]},{"Count":[{"Subtype":"Zombie"},{"In":"Graveyard"},{"OwnedBy":"You"}]}]}}}}}"#;
const ELENORA: &str = r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"Opponents"},{"Targeting":["This"]}],"change":"Increase","mana":"{2}"}}"#;
const REAPER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"},{"HasCounter":"-1/-1"}],"from":"Battlefield","to":"Graveyard"}},"limit":"OncePerTurn","effects":[{"May":{"effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{"under":"You"}}}}]}}]}}"#;
const VERIX: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":"Kicked","effects":[{"Draw":{}}]}}"#;
const TORRENT: &str = r#"{"Spell":{"effects":[{"DealDamage":{"to":{"Each":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},{"ControlledBy":"Opponents"}]},"amount":{"Tallied":{"who":"You","what":"SpellsCast"}}}}]}}"#;
const SIGARDA_ATTACH: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Subtype":"Equipment"},{"ControlledBy":"You"}],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"May":{"effects":[{"Attach":{"what":"Triggering","to":{"Target":0}}}]}}]}}"#;
const TRANSPOSE: &str = r#"{"Spell":{"effects":[{"Draw":{}},{"LoseLife":{"amount":1}},{"If":{"condition":{"Matches":{"subject":"This","selector":[{"CastFrom":"Hand"}]}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Wizard","colors":["Black"],"types":["Creature"],"subtypes":["Wizard"],"power":0,"toughness":1}}}}]}}]}}"#;
const REGROWTH: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":3}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"tapped":true}}}},{"CreateToken":{"token":{"Custom":{"name":"Plant Warrior","colors":["Green"],"types":["Creature"],"subtypes":["Plant","Warrior"],"power":4,"toughness":2,"abilities":[{"Keyword":"Reach"}]}},"count":{"TargetCount":0}}}]}}"#;
const THWART: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}},{"what":{"Object":[{"Type":"Creature"},{"AnyOf":[{"Subtype":"Cleric"},{"Subtype":"Rogue"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}},{"Move":{"what":{"Target":1},"to":{"Battlefield":{}}}}]}}"#;
const YAROK: &str =
    r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"ZoneChange":{"what":[],"to":"Battlefield"}}}}}}"#;
const VERGE: &str = r#"{"Rule":{"rule":{"MayCast":{"what":[{"Type":"Land"}],"from":"Library","play":true}},"condition":{"Compare":{"left":{"Largest":{"who":"Opponents","of":[{"Type":"Land"},{"ControlledBy":"You"}]}},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}}}"#;
const VEIN: &str = r#"{"Activated":{"cost":[{"Mana":"{R}"},"Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":"This","stat":"Power"}}}},{"DealDamage":{"source":{"Target":0},"to":"This","amount":{"Stat":{"of":{"Target":0},"stat":"Power"}}}}]}}"#;
const SALVO: &str = r#"{"Spell":{"targets":[{"what":{"Either":[{"Object":[{"Type":"Creature"}]},{"Object":[{"Type":"Planeswalker"}]}]},"bounds":{"min":0,"max":2}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":6}}]}}"#;
const SOULHERDER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Exile"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#;
const CERATOPS: &str = r#"{"Activated":{"cost":"{G}","effects":[{"Modal":{"modes":[{"effects":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Reach"}],"duration":"UntilEndOfTurn"}}]},{"effects":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}}]},{"effects":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}]}}]}}"#;
const STAFF: &str = r#"{"Activated":{"cost":["Tap",{"PayLife":1}],"targets":[{"what":{"Object":[{"OwnedBy":"You"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;

#[rstest]
#[case::sacrifice(
    GITROG,
    "At the beginning of your upkeep, sacrifice ~ unless you sacrifice a land."
)]
#[case::flash(YEVA, "You may cast green creature spells as though they had flash.")]
#[case::flash_union(
    SIGARDA,
    "You may cast Aura and Equipment spells as though they had flash."
)]
#[case::battalion(
    TAJIC,
    "Whenever ~ and at least two other creatures attack, ~ gets +5/+5 until end of turn."
)]
#[case::lower_case_duration(
    CICADA,
    "Whenever you cast a colorless spell, until end of turn, ~ gains trample and gets +X/+X, where X is that spell's mana value."
)]
#[case::returns_itself(
    SPIT_FLAME,
    "Whenever a Dragon you control enters, you may pay {R}. If you do, return this card from your graveyard to your hand."
)]
#[case::three_colors(
    SAND_SCOUT,
    "Whenever one or more land cards are put into your graveyard from anywhere, create a 1/1 red, green, and white Sand Warrior creature token. This ability triggers only once each turn."
)]
#[case::half_of_x(GOOSE, "When ~ enters, create half X Food tokens, rounded up.")]
#[case::excluded_supertype(BLAST, "Exile all nonland permanents that aren't legendary.")]
#[case::plus_one_of_each_kind(
    CONSTRICTOR,
    "If one or more counters would be put on an artifact or creature you control, that many plus one of each of those kinds of counters are put on that permanent instead."
)]
#[case::you_get_counters(
    CONSTRICTOR_YOU,
    "If you would get one or more counters, you get that many plus one of each of those kinds of counters instead."
)]
#[case::destroy_the_dealer(
    TEYSA,
    "Whenever a creature deals combat damage to you, destroy that creature."
)]
#[case::card_to_your_graveyard(
    SKOLA,
    "Whenever a land card is put into your graveyard from anywhere, you gain 1 life."
)]
#[case::dealt_damage(
    DROPLET,
    "Whenever you're dealt damage, put that many charge counters on this artifact."
)]
#[case::at_least(
    SVYELUN,
    "~ has indestructible as long as you control at least two other Merfolk."
)]
#[case::comma_in_the_quote(
    NOULITHS,
    "Equipped creature gets +1/+0, has \"Whenever this creature attacks, untap target attacking creature,\" and is a Cleric in addition to its other types."
)]
#[case::entry_and_step(
    TAIL,
    "When this creature enters and at the beginning of your upkeep, surveil 1."
)]
#[case::free_cast(
    SURTLAND,
    "Whenever this creature attacks, you may cast an instant or sorcery spell from your hand without paying its mana cost."
)]
#[case::choose_then_double(
    SOLIDARITY,
    "Choose any number of target creatures. Double the number of +1/+1 counters on each of them."
)]
#[case::prevent_and_remove(
    HORDE_DAMAGE,
    "If this creature would be dealt damage, prevent that damage and remove a +1/+1 counter from it."
)]
#[case::two_counts(
    HORDE_ENTERS,
    "This creature enters with a +1/+1 counter on it for each other Zombie you control and each Zombie card in your graveyard."
)]
#[case::targeted_spells(
    ELENORA,
    "Spells your opponents cast that target ~ cost {2} more to cast."
)]
#[case::put_that_card(
    REAPER,
    "Whenever a creature an opponent controls with a -1/-1 counter on it dies, you may put that card onto the battlefield under your control. Do this only once each turn."
)]
#[case::kicked(VERIX, "When ~ enters, if it was kicked, draw a card.")]
#[case::spells_cast(
    TORRENT,
    "~ deals X damage to each creature and planeswalker your opponents control, where X is the number of spells you've cast this turn."
)]
#[case::attach(
    SIGARDA_ATTACH,
    "Whenever an Equipment you control enters, you may attach it to target creature you control."
)]
#[case::cast_from_hand(
    TRANSPOSE,
    "Draw a card. You lose 1 life. If this spell was cast from your hand, create a 0/1 black Wizard creature token."
)]
#[case::that_many(
    REGROWTH,
    "Return up to three target land cards from your graveyard to the battlefield tapped. Create that many 4/2 green Plant Warrior creature tokens with reach."
)]
#[case::two_returns(
    THWART,
    "Return target creature card and up to one target Cleric or Rogue creature card from your graveyard to the battlefield."
)]
#[case::trigger_again(
    YAROK,
    "If a permanent entering causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
)]
#[case::rule_under_a_condition(
    VERGE,
    "As long as an opponent controls more lands than you, you may play lands from the top of your library."
)]
#[case::damage_both_ways(
    VEIN,
    "{R}, {T}: ~ deals damage equal to its power to target creature. That creature deals damage equal to its power to ~."
)]
#[case::each_of_two_kinds(
    SALVO,
    "~ deals 6 damage to each of up to two target creatures and/or planeswalkers."
)]
#[case::exiled_from_the_battlefield(
    SOULHERDER,
    "Whenever a creature is exiled from the battlefield, put a +1/+1 counter on ~."
)]
#[case::choice_of_keywords(
    CERATOPS,
    "{G}: ~ gains your choice of reach, trample, or haste until end of turn."
)]
#[case::permanent_you_own(STAFF, "{T}, Pay 1 life: Destroy target permanent you own.")]
fn the_wording_of_the_card_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_kind(
    GITROG,
    "At the beginning of your upkeep, sacrifice ~ unless you sacrifice a creature."
)]
#[case::another_step(
    GITROG,
    "At the beginning of each upkeep, sacrifice ~ unless you sacrifice a land."
)]
#[case::another_cost(
    GITROG,
    "At the beginning of your upkeep, sacrifice ~ unless you discard a land."
)]
#[case::another_color(YEVA, "You may cast red creature spells as though they had flash.")]
#[case::no_color(YEVA, "You may cast creature spells as though they had flash.")]
#[case::no_flash(YEVA, "You may cast green creature spells.")]
#[case::another_player(
    YEVA,
    "Your opponents may cast green creature spells as though they had flash."
)]
#[case::only_one_kind(SIGARDA, "You may cast Aura spells as though they had flash.")]
#[case::more_attackers(
    TAJIC,
    "Whenever ~ and at least three other creatures attack, ~ gets +5/+5 until end of turn."
)]
#[case::blockers(
    TAJIC,
    "Whenever ~ and at least two other creatures block, ~ gets +5/+5 until end of turn."
)]
#[case::another_bonus(
    TAJIC,
    "Whenever ~ and at least two other creatures attack, ~ gets +4/+4 until end of turn."
)]
#[case::another_keyword(
    CICADA,
    "Whenever you cast a colorless spell, until end of turn, ~ gains haste and gets +X/+X, where X is that spell's mana value."
)]
#[case::another_kind_of_spell(
    CICADA,
    "Whenever you cast a red spell, until end of turn, ~ gains trample and gets +X/+X, where X is that spell's mana value."
)]
#[case::to_the_battlefield(
    SPIT_FLAME,
    "Whenever a Dragon you control enters, you may pay {R}. If you do, return this card from your graveyard to the battlefield."
)]
#[case::another_type(
    SPIT_FLAME,
    "Whenever a Goblin you control enters, you may pay {R}. If you do, return this card from your graveyard to your hand."
)]
#[case::another_cost_to_return(
    SPIT_FLAME,
    "Whenever a Dragon you control enters, you may pay {G}. If you do, return this card from your graveyard to your hand."
)]
#[case::another_colors(
    SAND_SCOUT,
    "Whenever one or more land cards are put into your graveyard from anywhere, create a 1/1 red, green, and blue Sand Warrior creature token. This ability triggers only once each turn."
)]
#[case::no_limit(
    SAND_SCOUT,
    "Whenever one or more land cards are put into your graveyard from anywhere, create a 1/1 red, green, and white Sand Warrior creature token."
)]
#[case::rounded_down(GOOSE, "When ~ enters, create half X Food tokens, rounded down.")]
#[case::the_whole_of_x(GOOSE, "When ~ enters, create X Food tokens, rounded up.")]
#[case::another_token(GOOSE, "When ~ enters, create half X Treasure tokens, rounded up.")]
#[case::lands_too(BLAST, "Exile all nonland permanents.")]
#[case::another_exclusion(BLAST, "Exile all nonland permanents that aren't artifacts.")]
#[case::plus_two(
    CONSTRICTOR,
    "If one or more counters would be put on an artifact or creature you control, that many plus two of each of those kinds of counters are put on that permanent instead."
)]
#[case::another_group(
    CONSTRICTOR,
    "If one or more counters would be put on an artifact or land you control, that many plus one of each of those kinds of counters are put on that permanent instead."
)]
#[case::an_opponent_gets(
    CONSTRICTOR_YOU,
    "If an opponent would get one or more counters, they get that many plus one of each of those kinds of counters instead."
)]
#[case::a_target(
    TEYSA,
    "Whenever a creature deals combat damage to you, destroy target creature."
)]
#[case::damage_to_a_permanent(
    TEYSA,
    "Whenever a creature deals combat damage to an opponent, destroy that creature."
)]
#[case::any_graveyard(
    SKOLA,
    "Whenever a land card is put into a graveyard from anywhere, you gain 1 life."
)]
#[case::another_card_type(
    SKOLA,
    "Whenever a creature card is put into your graveyard from anywhere, you gain 1 life."
)]
#[case::from_the_battlefield(
    SKOLA,
    "Whenever a land card is put into your graveyard from the battlefield, you gain 1 life."
)]
#[case::combat_damage(
    DROPLET,
    "Whenever you're dealt combat damage, put that many charge counters on this artifact."
)]
#[case::an_opponent_is_dealt(
    DROPLET,
    "Whenever an opponent is dealt damage, put that many charge counters on this artifact."
)]
#[case::more_other_creatures(
    SVYELUN,
    "~ has indestructible as long as you control at least three other Merfolk."
)]
#[case::not_other(
    SVYELUN,
    "~ has indestructible as long as you control at least two Merfolk."
)]
#[case::the_period_inside(
    NOULITHS,
    "Equipped creature gets +1/+0, has \"Whenever this creature attacks, untap target attacking creature.\" and is a Cleric in addition to its other types."
)]
#[case::another_kind_of_creature(
    NOULITHS,
    "Equipped creature gets +1/+0, has \"Whenever this creature attacks, untap target attacking creature,\" and is a Wizard in addition to its other types."
)]
#[case::each_upkeep(
    TAIL,
    "When this creature enters and at the beginning of each upkeep, surveil 1."
)]
#[case::attacks_too(TAIL, "When this creature enters or attacks, surveil 1.")]
#[case::from_the_graveyard(
    SURTLAND,
    "Whenever this creature attacks, you may cast an instant or sorcery spell from your graveyard without paying its mana cost."
)]
#[case::one_kind(
    SURTLAND,
    "Whenever this creature attacks, you may cast an instant spell from your hand without paying its mana cost."
)]
#[case::must_cast(
    SURTLAND,
    "Whenever this creature attacks, cast an instant or sorcery spell from your hand without paying its mana cost."
)]
#[case::each_of_them_alone(SOLIDARITY, "Double the number of +1/+1 counters on each of them.")]
#[case::triple(
    SOLIDARITY,
    "Choose any number of target creatures. Triple the number of +1/+1 counters on each of them."
)]
#[case::another_counter(
    HORDE_DAMAGE,
    "If this creature would be dealt damage, prevent that damage and remove a -1/-1 counter from it."
)]
#[case::all_players(
    HORDE_DAMAGE,
    "If damage would be dealt to a creature, prevent that damage and remove a +1/+1 counter from it."
)]
#[case::not_other_zombies(
    HORDE_ENTERS,
    "This creature enters with a +1/+1 counter on it for each Zombie you control and each Zombie card in your graveyard."
)]
#[case::another_card(
    HORDE_ENTERS,
    "This creature enters with a +1/+1 counter on it for each other Zombie you control and each Elf card in your graveyard."
)]
#[case::your_own(ELENORA, "Spells you cast that target ~ cost {2} more to cast.")]
#[case::another_amount(
    ELENORA,
    "Spells your opponents cast that target ~ cost {1} more to cast."
)]
#[case::less(
    ELENORA,
    "Spells your opponents cast that target ~ cost {2} less to cast."
)]
#[case::under_its_owners_control(
    REAPER,
    "Whenever a creature an opponent controls with a -1/-1 counter on it dies, you may put that card onto the battlefield under its owner's control. Do this only once each turn."
)]
#[case::not_limited(
    REAPER,
    "Whenever a creature an opponent controls with a -1/-1 counter on it dies, you may put that card onto the battlefield under your control."
)]
#[case::not_kicked(VERIX, "When ~ enters, if it was cast, draw a card.")]
#[case::any_time(VERIX, "When ~ enters, draw a card.")]
#[case::spells_opponents_cast(
    TORRENT,
    "~ deals X damage to each creature and planeswalker your opponents control, where X is the number of spells your opponents have cast this turn."
)]
#[case::your_creatures(
    TORRENT,
    "~ deals X damage to each creature and planeswalker you control, where X is the number of spells you've cast this turn."
)]
#[case::attach_to_an_opponents(
    SIGARDA_ATTACH,
    "Whenever an Equipment you control enters, you may attach it to target creature an opponent controls."
)]
#[case::must_attach(
    SIGARDA_ATTACH,
    "Whenever an Equipment you control enters, attach it to target creature you control."
)]
#[case::cast_from_a_graveyard(
    TRANSPOSE,
    "Draw a card. You lose 1 life. If this spell was cast from your graveyard, create a 0/1 black Wizard creature token."
)]
#[case::as_many_as_two(
    REGROWTH,
    "Return up to three target land cards from your graveyard to the battlefield tapped. Create two 4/2 green Plant Warrior creature tokens with reach."
)]
#[case::one_target_only(
    THWART,
    "Return target creature card from your graveyard to the battlefield."
)]
#[case::to_your_hand(
    THWART,
    "Return target creature card and up to one target Cleric or Rogue creature card from your graveyard to your hand."
)]
#[case::a_creature_entering(
    YAROK,
    "If a creature entering causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
)]
#[case::an_opponents_permanent(
    YAROK,
    "If a permanent entering causes a triggered ability of a permanent an opponent controls to trigger, that ability triggers an additional time."
)]
#[case::without_the_condition(VERGE, "You may play lands from the top of your library.")]
#[case::fewer_lands(
    VERGE,
    "As long as an opponent controls fewer lands than you, you may play lands from the top of your library."
)]
#[case::no_return_damage(
    VEIN,
    "{R}, {T}: ~ deals damage equal to its power to target creature."
)]
#[case::another_stat(
    VEIN,
    "{R}, {T}: ~ deals damage equal to its power to target creature. That creature deals damage equal to its toughness to ~."
)]
#[case::one_kind_only(SALVO, "~ deals 6 damage to each of up to two target creatures.")]
#[case::three_targets(
    SALVO,
    "~ deals 6 damage to each of up to three target creatures and/or planeswalkers."
)]
#[case::exiled_from_a_graveyard(
    SOULHERDER,
    "Whenever a creature is exiled from a graveyard, put a +1/+1 counter on ~."
)]
#[case::dies(
    SOULHERDER,
    "Whenever a creature is put into a graveyard from the battlefield, put a +1/+1 counter on ~."
)]
#[case::a_keyword_too_many(
    CERATOPS,
    "{G}: ~ gains your choice of reach, trample, haste, or flying until end of turn."
)]
#[case::all_of_them(CERATOPS, "{G}: ~ gains reach, trample, and haste until end of turn.")]
#[case::another_permanent(
    STAFF,
    "{T}, Pay 1 life: Destroy target permanent an opponent controls."
)]
#[case::any_permanent(STAFF, "{T}, Pay 1 life: Destroy target permanent.")]
fn another_wording_does_not_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}
