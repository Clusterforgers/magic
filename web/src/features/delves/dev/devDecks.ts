/** The decks `/dev/delve` offers: a few of the player's own and some precons. */
export async function devDecks() {
  return [
    { value: 'd-atraxa', label: 'Atraxa Counters (yours)' },
    { value: 'precon:cmm', label: 'Draconic Dissent' },
    { value: 'precon:x', label: 'Elven Council' },
  ]
}
