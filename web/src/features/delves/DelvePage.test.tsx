// Goal: the run page shows each state of a run and sends the matching action: the lobby's hero cards with their Ready state, the deck picker, bot ally and start (only for the host, once every hero is ready), the ladder's votes with who voted for what, the boss fight's link and phases, how a lost fight cost a heart and at what life the heroes regroup (full at the first boss, half later), the host's retreat (confirmed first, never for a guest), every kind of offer (the Mirror Pool from the view's list), and the end.
import { fireEvent, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderInRouter } from '@/test-utils/router'
import { DelvePage } from './DelvePage'
import { delveView, fakeDelves, hero } from './fakeDelves'
import { pickerDecks } from './fakePickerDecks'
import type { DelveView, Offer } from '@/protocol/generated'

const noPictures = async () => null

async function show(view: DelveView) {
  const client = fakeDelves(view)
  await renderInRouter(<DelvePage id="d1" client={client} loadDecks={pickerDecks} find={noPictures} />)
  return client
}

const bo = hero({ name: 'Bot', user: null, bot: 'Normal' })
const ready = { deck: { id: 'precon:x', name: 'Elven Council', colors: 'G', commanders: [] } }

describe('the lobby', () => {
  it('shows each hero ready or choosing and lets the host descend only when all are ready', async () => {
    await show(delveView({}, { heroes: [hero(), hero({ ...bo, ...ready })] }))
    expect(await screen.findByRole('listitem', { name: 'Ana (you): choosing a deck' })).toHaveAttribute(
      'data-ready',
      'false',
    )
    expect(screen.getByRole('listitem', { name: 'Bot: ready with Elven Council' })).toHaveAttribute(
      'data-ready',
      'true',
    )
    expect(screen.getByText(/heroes ready/).parentElement).toHaveTextContent('1 of 2 heroes ready')
    expect(screen.getByRole('button', { name: 'Descend' })).toBeDisabled()
  })

  it('opens the deck picker and sends the deck taken', async () => {
    const client = await show(delveView({}, { heroes: [hero(), bo] }))
    fireEvent.click(await screen.findByRole('button', { name: 'Choose a deck for Ana' }))
    const dialog = await screen.findByRole('dialog', { name: 'Choose a deck for Ana' })
    const take = within(dialog).getByRole('button', { name: /^Take a deck/ })
    expect(take).toBeDisabled()
    fireEvent.click(await within(dialog).findByRole('button', { name: /Goblin Swarm/ }))
    fireEvent.click(within(dialog).getByRole('button', { name: 'Take Goblin Swarm for Ana' }))
    expect(client.actions).toEqual([{ kind: 'choose_deck', hero: 0, deck: 'd-krenko' }])
    const add = screen.getByRole('button', { name: 'Add a bot ally' })
    await waitFor(() => expect(add).toBeEnabled())
    fireEvent.click(add)
    expect(client.actions.at(-1)).toEqual({ kind: 'add_bot', level: 'Normal' })
    expect(screen.getByRole('button', { name: 'Choose a deck for Bot' })).toBeInTheDocument()
  })

  it('shows a guest no host controls and no other player’s deck choice', async () => {
    await show(delveView({ host: false, me: 1 }, { heroes: [hero(), hero({ name: 'Bo', user: 'u1' })] }))
    await screen.findByText(/The host leads the party in/)
    expect(screen.queryByRole('button', { name: 'Descend' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Choose a deck for Ana' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Choose a deck for Bo' })).toBeInTheDocument()
  })
})

describe('the ladder and the fight', () => {
  it('takes a vote for each camp after the boss and shows who voted for which', async () => {
    const client = await show(
      delveView(
        {},
        { status: 'Map', at: 0, cleared: [0], votes: { 1: 2 }, heroes: [hero(), hero({ name: 'Bo' })] },
      ),
    )
    const shop = await screen.findByRole('listitem', { name: 'Shop, Open to a vote' })
    const event = screen.getByRole('listitem', { name: 'Event, Open to a vote' })
    expect(within(event).getByText('Bo')).toBeInTheDocument()
    expect(within(shop).getByText('No votes yet')).toBeInTheDocument()
    expect(screen.getByText(/voted ·/)).toHaveTextContent('1 of 2 voted')
    const vote = screen.getByRole('region', { name: 'Vote for the next room' })
    fireEvent.click(within(vote).getByRole('button', { name: 'Vote for Shop' }))
    expect(client.actions).toEqual([{ kind: 'vote', node: 1 }])
  })

  it('marks the player’s own vote', async () => {
    await show(delveView({}, { status: 'Map', at: 0, cleared: [0], votes: { 0: 1 } }))
    expect(await screen.findByRole('button', { name: 'Your vote: Shop' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    const shop = screen.getByRole('listitem', { name: 'Shop, Open to a vote' })
    expect(within(shop).getByText('Your vote')).toBeInTheDocument()
    expect(screen.getByRole('listitem', { name: /^Boss: Skrix, the Rat Queen, Cleared/ })).toBeInTheDocument()
    expect(screen.getByRole('listitem', { name: /^Final boss: Grimgut, Locked/ })).toBeInTheDocument()
    // Each boss shows the life it starts with against this party at its place on the ladder.
    const final = screen.getByRole('listitem', { name: /^Final boss: Grimgut/ })
    expect(within(final).getByText('33 life')).toBeInTheDocument()
  })

  const fight = {
    boss: 'Skrix, the Rat Queen',
    life: 120,
    rules: ['skrix'],
    phases: [['nest', 80]] as Array<[string, number]>,
    minions: [],
    intensify: [] as string[],
  }

  it('opens with the boss intro: the way in, the party with its decks and life, the abilities and phases', async () => {
    await show(delveView({ boss: 'skrix_nest', fight }, { status: { Fight: { game: 'g42' } }, at: 0 }))
    const link = await screen.findByRole('link', { name: 'Enter the fight' })
    expect(link).toHaveAttribute('href', '/game/g42')
    const abilities = within(screen.getByRole('list', { name: 'Boss abilities' }))
    expect(abilities.getByText('Phase II — The Nest Stirs')).toBeInTheDocument()
    expect(abilities.getByText(/At 80 life \(67%\)/)).toBeInTheDocument()
    expect(abilities.getByText('More rats.')).toBeInTheDocument()
    expect(screen.getByRole('listitem', { name: /Skrix, the Rat Queen, You are here/ })).toHaveTextContent(
      'Fighting',
    )
    expect(screen.queryByRole('button', { name: /Choose a deck/ })).toBeNull()
  })

  it('holds a fight whose hero deck no longer plays, and offers that hero a new deck', async () => {
    const client = await show(
      delveView(
        { boss: 'skrix_nest', fight, deck_problems: [{ hero: 0, reason: 'the deck is gone' }] },
        { status: { Fight: { game: null } }, at: 0, heroes: [hero()] },
      ),
    )
    expect(await screen.findByText(/Deck problem: the deck is gone/)).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Enter the fight' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Choose another deck' }))
    const dialog = await screen.findByRole('dialog', { name: 'Choose a deck for Ana' })
    fireEvent.click(await within(dialog).findByRole('button', { name: /Goblin Swarm/ }))
    fireEvent.click(within(dialog).getByRole('button', { name: 'Take Goblin Swarm for Ana' }))
    await waitFor(() => expect(client.actions[0]).toMatchObject({ kind: 'choose_deck', hero: 0 }))
  })

  it.each([
    ['The party was beaten back and regroups (2 hearts left).', /beaten back and lost a heart \(2 left\)/],
    ['The party retreated and regroups (2 hearts left).', /retreated and lost a heart \(2 left\)/],
    [
      'The party lost its table and regroups (2 hearts left).',
      /table closed without a result, which counts as a lost fight: the party lost a heart \(2 left\)/,
    ],
  ])('says how a lost fight cost a heart: %s', async (line, note) => {
    await show(
      delveView(
        { boss: 'skrix_nest', fight },
        { status: { Fight: { game: 'g43' } }, at: 0, hearts: 2, log: [line] },
      ),
    )
    expect(await screen.findByText(note)).toBeInTheDocument()
  })

  it.each([
    ['the first boss', 0, 'full'],
    ['a later boss', 3, 'half'],
  ])(
    'says the heroes regroup at %s: %s life, in the loss note and the retreat dialog',
    async (_boss, at, life) => {
      await show(
        delveView(
          { boss: 'skrix_nest', fight },
          {
            status: { Fight: { game: 'g47' } },
            at,
            hearts: 2,
            log: ['The party was beaten back and regroups (2 hearts left).'],
          },
        ),
      )
      const note = await screen.findByText(/beaten back and lost a heart/)
      expect(note).toHaveTextContent(`Every hero regroups at ${life} life for another try.`)
      fireEvent.click(screen.getByRole('button', { name: 'Retreat' }))
      const dialog = await screen.findByRole('alertdialog', { name: 'Retreat from the fight?' })
      expect(dialog).toHaveTextContent(`every hero regroups at ${life} life and the boss`)
    },
  )

  it('leaves the loss note out while the last line of the chronicle is something else', async () => {
    await show(
      delveView(
        { boss: 'skrix_nest', fight },
        { status: { Fight: { game: 'g43' } }, at: 0, hearts: 2, log: ['Skrix awaits.'] },
      ),
    )
    await screen.findByRole('link', { name: 'Enter the fight' })
    expect(screen.queryByText(/lost a heart/)).toBeNull()
  })

  it("puts the way in and the host's Retreat on one row, the way in first", async () => {
    await show(delveView({ boss: 'skrix_nest', fight }, { status: { Fight: { game: 'g45' } }, at: 0 }))
    const link = await screen.findByRole('link', { name: 'Enter the fight' })
    const row = link.parentElement
    expect(row).toHaveAttribute('data-slot', 'fight-actions')
    expect(row?.children[1]).toBe(screen.getByRole('button', { name: 'Retreat' }))
  })

  it('lets the host retreat from the fight after a confirmation, and only then sends it', async () => {
    const client = await show(
      delveView({ boss: 'skrix_nest', fight }, { status: { Fight: { game: 'g44' } }, at: 0, hearts: 3 }),
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Retreat' }))
    const dialog = await screen.findByRole('alertdialog', { name: 'Retreat from the fight?' })
    expect(dialog).toHaveTextContent('the party loses a heart (2 left)')
    fireEvent.click(within(dialog).getByRole('button', { name: 'Keep fighting' }))
    expect(client.actions).toEqual([])
    fireEvent.click(screen.getByRole('button', { name: 'Retreat' }))
    const again = await screen.findByRole('alertdialog', { name: 'Retreat from the fight?' })
    fireEvent.click(within(again).getByRole('button', { name: 'Retreat' }))
    await waitFor(() => expect(client.actions).toEqual([{ kind: 'retreat' }]))
  })

  it('warns the host that retreating with the last heart ends the run', async () => {
    await show(
      delveView({ boss: 'skrix_nest', fight }, { status: { Fight: { game: 'g45' } }, at: 0, hearts: 1 }),
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Retreat' }))
    expect(await screen.findByRole('alertdialog')).toHaveTextContent('last heart')
  })

  it('offers no retreat to a guest, and none outside a fight', async () => {
    await show(
      delveView(
        { host: false, me: 0, boss: 'skrix_nest', fight },
        { status: { Fight: { game: 'g46' } }, at: 0 },
      ),
    )
    await screen.findByRole('link', { name: 'Enter the fight' })
    expect(screen.queryByRole('button', { name: 'Retreat' })).toBeNull()
  })

  it('offers the host no retreat at a camp', async () => {
    await show(delveView({ boss: 'skrix_nest' }, { status: 'Map', at: 0, cleared: [0] }))
    await screen.findByRole('button', { name: 'End the run' })
    expect(screen.queryByRole('button', { name: 'Retreat' })).toBeNull()
  })

  it('draws End the run as an outline button, as Retreat is, not as plain text', async () => {
    await show(delveView({ boss: 'skrix_nest' }, { status: 'Map', at: 0, cleared: [0] }))
    const end = await screen.findByRole('button', { name: 'End the run' })
    expect(end).toHaveAttribute('data-slot', 'end-run')
    expect(end).toHaveClass('border-border-strong', 'justify-self-start')
    expect(end.querySelector('svg')).not.toBeNull()
  })

  it('greets the camp after a boss with its fall', async () => {
    await show(delveView({ boss: 'skrix_nest' }, { status: 'Room', at: 0, cleared: [0] }))
    expect(await screen.findByText(/Skrix, the Rat Queen falls!/)).toBeInTheDocument()
  })
})

describe('offers', () => {
  const room = (offer: Offer, gold = 0) =>
    delveView({}, { status: 'Room', at: 0, cleared: [0], heroes: [hero({ offers: [offer], gold })] })

  it.each([
    [{ Relics: { relics: ['idol', 'pact'] } } as Offer, 'Take Ember Idol', { Take: 0 }],
    [{ Cards: { cards: ['Lightning Bolt'] } } as Offer, 'Take Lightning Bolt', { Take: 0 }],
    [{ Campfire: { upgrades: [] } } as Offer, 'Rest', 'Rest'],
    [{ Campfire: { upgrades: [] } } as Offer, 'Reforge', 'Reforge'],
    [{ Event: { event: 'toll' } } as Offer, 'Turn back', { Take: 3 }],
    [{ Shop: { items: ['Draught'] } } as Offer, 'Leave the shop', 'Skip'],
  ])('answers %j with %s', async (offer, button, pick) => {
    const client = await show(room(offer, 100))
    fireEvent.click(await screen.findByRole('button', { name: button }))
    expect(client.actions).toEqual([{ kind: 'pick', pick }])
  })

  it('answers the Mirror Pool with a card of the view’s list', async () => {
    const view = { ...room('Duplicate'), mirror: ['Cultivate', 'Sol Ring'] }
    const client = await show(view)
    fireEvent.click(await screen.findByRole('button', { name: 'Copy Sol Ring' }))
    expect(client.actions).toEqual([{ kind: 'pick', pick: { Duplicate: 'Sol Ring' } }])
  })

  it('marks a risky relic as dangerous and keeps an unaffordable ware out of reach', async () => {
    await show(room({ Relics: { relics: ['idol', 'pact'] } }))
    const pact = await screen.findByRole('figure', { name: 'Wildfire Pact' })
    expect(within(pact).getByText('Dangerous')).toBeInTheDocument()
    expect(within(screen.getByRole('figure', { name: 'Ember Idol' })).queryByText('Dangerous')).toBeNull()
  })

  it('keeps an unaffordable ware out of reach and says what it needs', async () => {
    await show(room({ Shop: { items: [{ Relic: { id: 'idol', price: 70, sold: false } }, 'Draught'] } }, 20))
    expect(await screen.findByRole('button', { name: 'Buy (70 gold)' })).toBeDisabled()
    expect(screen.getByRole('button', { name: /Healing draught/ })).toBeDisabled()
    expect(screen.getByText('Needs 70 gold')).toBeInTheDocument()
  })

  it('stands every Buy button of the shop at the foot of its offer, so a row of offers buys on one line', async () => {
    await show(
      room(
        {
          Shop: {
            items: [
              { Card: { name: 'Cultivate', sold: false } },
              { Relic: { id: 'idol', price: 40, sold: false } },
              'Draught',
            ],
          },
        },
        100,
      ),
    )
    await screen.findByRole('button', { name: 'Buy (40 gold)' })
    const offers = [...document.querySelectorAll<HTMLElement>('[data-slot="shop-offer"]')]
    expect(offers).toHaveLength(3)
    for (const offer of offers) {
      expect(offer).toHaveClass('flex', 'flex-col')
      // The offer's last element is its button, outside any frame, with the rest stretched or pushed above it.
      const button = must(offer.querySelector('button:last-of-type'))
      expect(offer.lastElementChild === button || offer.lastElementChild?.lastElementChild === button).toBe(
        true,
      )
    }
    expect(screen.getByRole('figure', { name: 'Ember Idol' }).querySelector('button')).toBeNull()
    expect(screen.getByRole('figure', { name: 'Ember Idol' })).toHaveClass('flex-1')
    expect(screen.getByRole('figure', { name: 'Cultivate' })).toHaveClass(
      'flex-1',
      'grid-rows-[auto_auto_1fr]',
    )
  })

  it('says what a campfire rest heals', async () => {
    await show(
      delveView(
        {},
        { status: 'Room', at: 0, heroes: [hero({ offers: [{ Campfire: { upgrades: [] } }], life: 12 })] },
      ),
    )
    expect(await screen.findByText('Heal 20 life: 12 → 32.')).toBeInTheDocument()
  })
})

it('names the end of a won run and its seed code', async () => {
  await show(delveView({}, { status: 'Victory', at: 3, cleared: [0, 1, 3] }))
  expect(await screen.findByRole('heading', { name: 'Victory!' })).toBeInTheDocument()
  expect(screen.getByText('SEED', { selector: 'strong' })).toBeInTheDocument()
  expect(screen.getAllByRole('button', { name: /Copy seed code/ }).length).toBeGreaterThan(0)
})

it.each([
  [['idol'], '1 relic (Ember Idol)'],
  [['idol', 'mend'], '2 relics (Ember Idol, mend)'],
  [[], '0 relics'],
])('counts a hero holding %j in the run summary as "%s"', async (relics, words) => {
  await show(delveView({}, { status: 'Victory', at: 3, cleared: [0, 1, 3], heroes: [hero({ relics })] }))
  const heroes = await screen.findByRole('list', { name: 'Heroes' })
  expect(heroes).toHaveTextContent(`40 life · 0 gold · ${words}`)
})

it('keeps each hero of the run summary on one row of name and figures, the relic names under them', async () => {
  await show(
    delveView({}, { status: 'Victory', at: 3, cleared: [0, 1, 3], heroes: [hero({ relics: ['idol'] })] }),
  )
  const row = must((await screen.findByRole('list', { name: 'Heroes' })).querySelector('li'))
  expect(row).toHaveClass('grid', 'grid-cols-[minmax(0,1fr)_auto]')
  expect(must(row.querySelector('[data-slot="summary-figures"]'))).toHaveClass('whitespace-nowrap')
  expect(row.querySelector('[data-slot="summary-figures"]')).not.toHaveTextContent('Ember Idol')
  expect(must(row.querySelector('[data-slot="summary-relics"]'))).toHaveClass('col-span-full')
})

describe('ending the run', () => {
  it('asks before the host ends it, and only then sends it', async () => {
    const client = await show(delveView({}, { status: 'Map', at: 0, cleared: [0] }))
    fireEvent.click(await screen.findByRole('button', { name: 'End the run' }))
    const dialog = await screen.findByRole('alertdialog', { name: 'End the run?' })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Keep going' }))
    expect(client.actions).toEqual([])
    fireEvent.click(screen.getByRole('button', { name: 'End the run' }))
    const again = await screen.findByRole('alertdialog', { name: 'End the run?' })
    fireEvent.click(within(again).getByRole('button', { name: 'End the run' }))
    await waitFor(() => expect(client.actions).toEqual([{ kind: 'abandon' }]))
  })
})
