//! Goal: scratch probe of wordings (deleted before the commit).
use super::*;

#[test]
fn probe() {
    let spells = [
        "Create X 1/1 white Pegasus creature tokens with flying, where X is your life total.",
        "Create a 3/3 green Beast creature token for each creature put into your graveyard from the battlefield this turn.",
        "For each land you control, create a Treasure token.",
        "Create three 1/1 colorless Eldrazi Scion creature tokens. They have \"Sacrifice this token: Add {C}.\"",
        "Each player other than target player creates a 5/5 red Dragon creature token with flying.",
        "Create a 1/1 colorless Phyrexian Mite artifact creature token with toxic 1 and \"This token can't block.\"",
        "Create two 1/1 black Rat creature tokens with \"This token can't block.\"",
        "Create a 2/2 black Zombie creature token with decayed.",
        "Create a 2/2 green Cat Warrior creature token with forestwalk.",
        "Create a 2/2 green Alien Salamander creature token with islandwalk.",
        "Create a 0/0 green and blue Fractal creature token. Put X +1/+1 counters on it, where X is the number of lands you control.",
        "Create a 1/1 white Soldier creature token, then attach this Equipment to it.",
        "Create a 1/1 red Goblin creature token. That token gains haste until end of turn.",
        "Create two 1/1 red Goblin creature tokens. Those tokens gain haste until end of turn.",
        "Create three tapped X/X green Treefolk creature tokens, where X is the number of lands you control. Put a reach counter on each of them.",
        "Create an X/X blue Illusion creature token, where X is the number of lands you control.",
        "Create a 1/1 colorless Triskelavite artifact creature token with flying. It has \"Sacrifice this token: This token deals 1 damage to any target.\"",
        "Choose an opponent. You and that player each create a 1/1 white Spirit creature token with flying.",
        "You and target opponent each create a Treasure token.",
    ];
    for text in spells {
        let read = clause(text, ClauseKind::Spell, false).is_some();
        println!("SPELL {} :: {text}", if read { "OK" } else { "NO" });
    }
    let triggers = [
        "When this creature dies, create X Junk tokens, where X is the number of +1/+1 counters on it.",
        "When this creature dies, create a number of tapped Treasure tokens equal to its power.",
        "Whenever this creature enters or attacks, create a number of 1/1 colorless Gnome artifact creature tokens equal to its power.",
        "When this creature enters, create a 2/2 green Bear creature token for each nontoken creature put into your graveyard from the battlefield this turn.",
        "Whenever you cast a noncreature spell, create an X/X blue Shark creature token with flying, where X is that spell's mana value.",
        "When this Equipment enters, create a 1/1 white Soldier creature token, then attach this Equipment to it.",
        "At the beginning of combat on your turn, if you control your commander, create a 1/1 colorless Thopter artifact creature token with flying. That token gains haste until end of turn.",
        "When this creature dies, create X 1/1 blue Squid creature tokens with islandwalk, where X is the number of +1/+1 counters on this creature.",
        "Whenever you cast or copy an instant or sorcery spell, create a 0/0 green and blue Fractal creature token. Put X +1/+1 counters on it, where X is that spell's mana value.",
        "At the beginning of your upkeep, you lose 1 life and create a 1/1 colorless Phyrexian Mite artifact creature token with toxic 1 and \"This token can't block.\"",
        "Whenever a land enchanted player controls enters, you create a 2/2 black Zombie creature token with decayed.",
        "At the beginning of your end step, create a number of 1/1 white Soldier creature tokens equal to the number of opponents who control more creatures than you.",
        "At the beginning of your upkeep, create a 1/1 green Phyrexian Insect creature token with infect for each poison counter your opponents have.",
        "Whenever this creature deals combat damage to a player, create that many 0/1 colorless Eldrazi Spawn creature tokens. They have \"Sacrifice this token: Add {C}.\"",
    ];
    for text in triggers {
        let read = clause(text, ClauseKind::Triggered, false).is_some();
        println!("TRIG {} :: {text}", if read { "OK" } else { "NO" });
    }
    let activated = [
        "{2}{R}, {T}, Sacrifice this artifact: Create an X/X colorless Golem artifact creature token with haste, where X is the number of charge counters on ~. Activate only as a sorcery.",
        "{4}{B}{B}, {T}, Sacrifice this artifact: Create a 2/2 black Zombie creature token with decayed for each corpse counter on ~.",
    ];
    for text in activated {
        let read = clause(text, ClauseKind::Activated, false).is_some();
        println!("ACT {} :: {text}", if read { "OK" } else { "NO" });
    }
}
