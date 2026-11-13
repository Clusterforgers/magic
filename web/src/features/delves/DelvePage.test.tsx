// Goal: the run page shows each state of a run and sends the matching action: the lobby's deck choice, bot ally and start (only for the host, once every hero has a deck), the ladder's votes on the next camps, the boss fight's link and phases, every kind of offer, and the end.
import { fireEvent, screen, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderInRouter } from '@/test-utils/router'
import { DelvePage } from './DelvePage'
import { delveView, fakeDelves, hero } from './fakeDelves'
import type { DelveView, Offer } from '@/protocol/generated'

const decks = async () => [{ value: 'precon:x', label: 'Precon X' }]

async function show(view: DelveView) {
  const client = fakeDelves(view)
  await renderInRouter(<DelvePage id="d1" client={client} loadDecks={decks} />)
  return client
}

describe('the lobby', () => {
  it('lets a player choose their deck and the host start once every hero has one', async () => {
    const client = await show(
      delveView({}, { heroes: [hero(), hero({ name: 'Bot', user: null, bot: 'Normal' })] }),
    )
    const descend = await screen.findByRole('button', { name: 'Descend' })
    expect(descend).toBeDisabled()
    await screen.findAllByRole('option', { name: 'Precon X' })
    fireEvent.change(screen.getByRole('combobox', { name: 'Deck for Ana' }), {
      target: { value: 'precon:x' },
    })
    expect(client.actions).toEqual([{ kind: 'choose_deck', hero: 0, deck: 'precon:x' }])
    const add = screen.getByRole('button', { name: 'Add a bot ally' })
    await waitFor(() => expect(add).toBeEnabled())
    fireEvent.click(add)
    expect(client.actions.at(-1)).toEqual({ kind: 'add_bot', level: 'Normal' })
    expect(screen.getByRole('combobox', { name: 'Deck for Bot' })).toBeInTheDocument()
  })

  it('shows a guest no host controls and no other player’s deck choice', async () => {
    await show(delveView({ host: false, me: 1 }, { heroes: [hero(), hero({ name: 'Bo', user: 'u1' })] }))
    await screen.findByText(/The host leads the party in/)
    expect(screen.queryByRole('button', { name: 'Descend' })).toBeNull()
    expect(screen.queryByRole('combobox', { name: 'Deck for Ana' })).toBeNull()
    expect(screen.getByRole('combobox', { name: 'Deck for Bo' })).toBeInTheDocument()
  })
})

describe('the ladder and the fight', () => {
  it('takes a vote for each camp after the boss and names the voters', async () => {
    const client = await show(
      delveView(
        {},
        { status: 'Map', at: 0, cleared: [0], votes: { 1: 2 }, heroes: [hero(), hero({ name: 'Bo' })] },
      ),
    )
    const votes = await screen.findAllByRole('button', { name: 'Vote' })
    expect(votes).toHaveLength(2)
    expect(screen.getByText('(Bo)')).toBeInTheDocument()
    fireEvent.click(votes[0]!)
    expect(client.actions).toEqual([{ kind: 'vote', node: 1 }])
  })

  it('links the fight’s table and shows the boss card with its phases', async () => {
    await show(delveView({ boss: 'skrix_nest' }, { status: { Fight: { game: 'g42' } }, at: 0 }))
    const link = await screen.findByRole('link', { name: 'Go to the fight' })
    expect(link).toHaveAttribute('href', '/game/g42')
    expect(screen.getByRole('figure', { name: 'Skrix, the Rat Queen' })).toBeInTheDocument()
    expect(screen.getByText('At 66% of its life')).toBeInTheDocument()
  })
})

describe('offers', () => {
  const room = (offer: Offer, gold = 0) =>
    delveView({}, { status: 'Room', at: 0, cleared: [0], heroes: [hero({ offers: [offer], gold })] })

  it.each([
    [{ Relics: { relics: ['idol', 'pact'] } } as Offer, 'Take', { Take: 0 }],
    [{ Cards: { cards: ['Lightning Bolt'] } } as Offer, 'Take Lightning Bolt', { Take: 0 }],
    ['Campfire' as Offer, 'Rest', 'Rest'],
    [{ Event: { event: 'toll' } } as Offer, 'Turn back', { Take: 1 }],
    [{ Shop: { items: ['Draught'] } } as Offer, 'Leave the shop', 'Skip'],
  ])('answers %j with %s', async (offer, button, pick) => {
    const client = await show(room(offer, 100))
    const buttons = await screen.findAllByRole('button', { name: button })
    fireEvent.click(buttons[0]!)
    expect(client.actions).toEqual([{ kind: 'pick', pick }])
  })

  it('marks a risky relic and keeps an unaffordable ware out of reach', async () => {
    await show(room({ Shop: { items: [{ Relic: { id: 'idol', price: 70, sold: false } }, 'Draught'] } }, 20))
    expect(await screen.findByRole('button', { name: 'Buy (70 gold)' })).toBeDisabled()
    expect(screen.getByRole('button', { name: /Healing draught/ })).toBeDisabled()
  })
})

it('names the end of a won run and its seed code', async () => {
  await show(delveView({}, { status: 'Victory', at: 3, cleared: [0, 1, 3] }))
  expect(await screen.findByRole('heading', { name: 'Victory!' })).toBeInTheDocument()
  expect(screen.getByText('SEED', { selector: 'strong' })).toBeInTheDocument()
})
