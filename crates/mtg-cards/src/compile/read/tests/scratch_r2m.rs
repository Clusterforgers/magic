//! Goal: scratch probe of what the reader does with the r2m wordings (removed before commit).
use super::*;

#[test]
fn probe() {
    let triggered = [
        "Whenever ~ attacks, choose one at random \u{2014}\n\u{2022} Thay \u{2014} Put a +1/+1 counter on each artifact creature you control.\n\u{2022} Caan \u{2014} Draw two cards.\n\u{2022} Sec \u{2014} Create a 3/3 black Dalek artifact creature token with menace.\n\u{2022} Jast \u{2014} Each opponent loses 4 life.",
        "At the beginning of combat on your turn, choose one at random \u{2014}\n\u{2022} Other creatures you control get +3/+0 and gain trample until end of turn.\n\u{2022} Discard your hand, then draw four cards.\n\u{2022} ~ deals 5 damage to any target.",
        "Whenever another creature you control enters, choose one that hasn't been chosen this turn.\n\u{2022} Put a +1/+1 counter on ~.\n\u{2022} Create a Food token.\n\u{2022} Scry 1.",
        "Whenever one or more cards leave your graveyard, choose one that hasn't been chosen this turn \u{2014}\n\u{2022} Draw a card.\n\u{2022} Create a Treasure token.\n\u{2022} Create a 2/2 black Zombie Druid creature token.",
        "Whenever one or more Dragons you control deal combat damage to an opponent, choose one that hasn't been chosen this turn \u{2014}\n\u{2022} Destroy target artifact that opponent controls.\n\u{2022} This creature deals 4 damage to each other opponent.\n\u{2022} Exile the top card of your library. You may play it this turn.",
        "Whenever one or more creatures you control deal combat damage to a player, you may put a permanent card with mana value 3 or less from your hand onto the battlefield.",
        "Whenever one or more Dragons you control deal combat damage to a player, you may put a permanent card with mana value less than or equal to that damage from your hand onto the battlefield.",
    ];
    for text in ["Put a +1/+1 counter on each artifact creature you control.", "Create a 3/3 black Dalek artifact creature token with menace.", "Each opponent loses 4 life."] {
        eprintln!("MODE {} :: {text}", clause(text, ClauseKind::Spell, false).is_some());
    }
    for text in triggered {
        let read = clause(text, ClauseKind::Triggered, false);
        eprintln!("PROBE {} :: {}", read.is_some(), text.chars().take(100).collect::<String>().replace('\n', " "));
        if let Some(read) = read {
            let wordings = render_variants(&read);
            let back = wordings.iter().any(|written| same_text(written, text));
            eprintln!("   back={back} {:?}", wordings.first().map(|w| w.chars().take(80).collect::<String>()));
        }
    }
}
