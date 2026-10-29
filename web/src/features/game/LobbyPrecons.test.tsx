// Goal: a guest in a lobby chooses a precon that the game's format takes (Commander precons in a Commander game, the rest in a Normal one), finds it by name, set or commander, sees the table's verdict on each shown precon before choosing, cannot press one the table cannot take, and sees the house rules of a listed game named in the lobby.
import { screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { legalityIssue, reportCard, reportOf, summary } from '@/features/decks/fakeDecks'
import { jsonResponse } from '@/test-utils/fakeFetch'
import type { ServerScript } from '@/test-utils/fakeServer'
import { renderGame } from '@/test-utils/gameHarness'
import { changesOf } from '@/test-utils/lobbyControls'
import { lobbyMessage } from '@/test-utils/lobbyFrames'
import type { SeatFlags } from '@/test-utils/lobbyFrames'
import type { HouseRules, PreconListItem } from '@/protocol/generated'

afterEach(() => window.sessionStorage.clear())

const TABLE: SeatFlags[] = [
  ['Ana', true, false, null],
  ['Bo', true, false],
]

const precon = (name: string, set: string, commanders: string[]): PreconListItem => {
  const code = `${set}-${name.toLowerCase().replace(/\W+/g, '-')}`
  return {
    code,
    name,
    set,
    deck_type: commanders.length > 0 ? 'Commander Deck' : 'Starter Deck',
    release_date: '2020-01-01',
    commanders,
    card_count: 100,
    format: commanders.length > 0 ? 'commander' : 'freeform',
    state: 'verified',
    verified: true,
    playable: true,
    summary: summary(),
  }
}

const PRECONS = [
  precon('Draconic Domination', 'c18', ['The Ur-Dragon']),
  precon('Merciless Rage', 'c19', ['Kediss, Emberclaw Familiar']),
  precon('Arcane Wizardry', 'c17', ['Inalla, Archmage Ritualist']),
  precon('Mono White Starter', 'st1', []),
]

// The verdict of the table on each: Merciless Rage has four cards to play by hand, Arcane Wizardry cannot be taken.
const REPORTS: Record<string, ReturnType<typeof reportOf>> = {
  'precon:c19-merciless-rage': reportOf([
    reportCard('Forest', { count: 96 }),
    reportCard('Mana Reflection', { count: 4, support: 'manual' }),
  ]),
  'precon:c17-arcane-wizardry': reportOf(
    [reportCard('Forest', { count: 99 })],
    [legalityIssue('The deck has 99 cards, and a Commander deck needs 100')],
  ),
}

interface Scene {
  format?: string
  precons?: PreconListItem[]
  script?: ServerScript
  visibility?: string
  rules?: HouseRules
}

async function openPrecons({
  format = 'commander',
  precons = PRECONS,
  rules,
  script = {},
  visibility = 'public',
}: Scene = {}) {
  const h = await renderGame({
    script: {
      precons: () => jsonResponse(200, { precons }),
      check: (body) =>
        jsonResponse(
          200,
          REPORTS[(body as { deck: string }).deck] ?? reportOf([reportCard('Forest', { count: 100 })]),
        ),
      game: () =>
        jsonResponse(200, {
          format,
          started: false,
          seats: TABLE.map(([name], seat) => ({ seat, name })),
          connected: [0, 1],
          ready: [],
          result: null,
          visibility,
          manual_play: false,
          ...(rules === undefined ? {} : { house_rules: rules }),
        }),
      ...script,
    },
  })
  h.join(0, TABLE.length)
  h.deliver(lobbyMessage(TABLE))
  await h.settle()
  return h
}

// The rows of the list: the support filter's own options are options too.
const deckRows = (): HTMLElement[] =>
  screen.queryAllByRole('option').filter((row) => row.hasAttribute('data-deck-option'))
const names = (): string[] => deckRows().map((item) => item.querySelector('span span')?.textContent ?? '')

// Choose the way a player does: select the row, then press Use this deck.
async function use(h: Awaited<ReturnType<typeof openPrecons>>, name: RegExp): Promise<void> {
  await h.user.click(await screen.findByRole('option', { name }))
  await h.user.click(await screen.findByRole('button', { name: 'Use this deck' }))
}

describe('the precons on offer', () => {
  it('lists the precons with a commander in a Commander game, and only a Commander tab', async () => {
    await openPrecons()
    expect(await screen.findByRole('listbox', { name: 'Precons' })).toBeInTheDocument()
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual(['Precons'])
    expect(names().map((text) => text.slice(0, 12))).toEqual(['Draconic Dom', 'Merciless Ra', 'Arcane Wizar'])
    expect(document.querySelector('[data-slot="deck-count"]')).toHaveTextContent('3 decks')
  })

  it('lists the precons without a commander in a Normal game, after the built-in decks', async () => {
    const h = await openPrecons({ format: 'normal' })
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual(['Built-in', 'Precons'])
    await h.user.click(screen.getByRole('tab', { name: 'Precons' }))
    await screen.findByRole('listbox', { name: 'Precons' })
    expect(names()).toHaveLength(1)
    expect(names()[0]).toMatch(/^Mono White Starter/)
  })

  it('finds a precon by name, set or commander, every word of the search at once', async () => {
    const h = await openPrecons()
    const search = await screen.findByRole('searchbox', { name: 'Search precons' })
    await h.user.type(search, 'ur-dragon')
    expect(names()).toHaveLength(1)
    expect(names()[0]).toMatch(/^Draconic Domination/)
    await h.user.clear(search)
    await h.user.type(search, 'c19 rage')
    expect(names()[0]).toMatch(/^Merciless Rage/)
    await h.user.clear(search)
    await h.user.type(search, 'nothing like this')
    expect(deckRows()).toHaveLength(0)
    expect(document.querySelector('[data-slot="deck-count"]')).toHaveTextContent('No deck matches.')
  })

  it('lists every precon and checks only the one selected, once it has rested, and again for the next', async () => {
    const many = Array.from({ length: 15 }, (_, n) =>
      precon(`Deck ${String(n).padStart(2, '0')}`, 'c20', ['A']),
    )
    const h = await openPrecons({ precons: many })
    await screen.findByRole('listbox', { name: 'Precons' })
    expect(names()).toHaveLength(15)
    const asked = () => h.server.calls.filter((call) => call.url === '/api/decks/check').length
    await waitFor(() => expect(asked()).toBe(1))
    await h.user.click(screen.getByRole('option', { name: /Deck 07/ }))
    await waitFor(() => expect(asked()).toBe(2))
  })

  it('shows the table’s verdict on the selected precon, and a precon the table cannot take cannot be used', async () => {
    const h = await openPrecons()
    expect(
      await screen.findByRole('option', { name: /Draconic Domination.*Whole deck runs/ }),
    ).toBeInTheDocument()
    await h.user.click(screen.getByRole('option', { name: /Merciless Rage/ }))
    expect(await screen.findByText('Legal in Commander')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Use this deck' })).toBeEnabled()
    await h.user.click(screen.getByRole('option', { name: /Arcane Wizardry/ }))
    expect(await screen.findByText('Not legal in Commander')).toBeInTheDocument()
    const blocked = screen.getByRole('button', { name: 'Use this deck' })
    await waitFor(() => expect(blocked).toHaveAttribute('aria-disabled', 'true'))
    expect(blocked).toHaveAccessibleDescription(/The deck has 99 cards, and a Commander deck needs 100\./)
    await h.user.click(blocked)
    expect(changesOf(h.server).filter((call) => call.method === 'PUT')).toEqual([])
  })

  it('chooses a precon with its code and says what the table now sees', async () => {
    const h = await openPrecons({
      script: {
        chooseDeck: () =>
          jsonResponse(200, { name: 'Draconic Domination', format: 'commander', manual_cards: 0 }),
      },
    })
    await use(h, /Draconic Domination/)
    await h.settle()
    const put = changesOf(h.server).filter((call) => call.method === 'PUT')
    expect(put).toHaveLength(1)
    expect(put[0]?.body).toEqual({ deck: 'precon:c18-draconic-domination' })
    expect(document.querySelector('[data-slot="deck-note"]')).toHaveTextContent(
      'Your deck is now Draconic Domination, Commander, no manual steps.',
    )
  })

  it('says why the list is missing and loads it again on request', async () => {
    let answers = 0
    const h = await openPrecons({
      script: {
        precons: () =>
          answers++ === 0
            ? jsonResponse(429, { error: 'rate_limited', message: 'slow down' })
            : jsonResponse(200, { precons: PRECONS }),
      },
    })
    expect(await screen.findByText(/too many requests/)).toBeInTheDocument()
    await h.user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(await screen.findByRole('listbox', { name: 'Precons' })).toBeInTheDocument()
  })
})

describe('the house rules in the lobby', () => {
  const RULES: HouseRules = { starting_life: 30, free_mulligans: 1, mulligan_style: 'vancouver' }

  it.each(['public', 'invite'])(
    'names the rules of a %s game that differ from the format’s own',
    async (visibility) => {
      await openPrecons({ rules: RULES, visibility })
      const line = await screen.findByText(/House rules:/)
      expect(line.parentElement).toHaveTextContent('Starting life 30 · Vancouver mulligan')
      // One free mulligan is Commander's own.
      expect(line.parentElement).not.toHaveTextContent('free mulligan')
    },
  )

  it.each([
    ['rules equal to the format’s own', { starting_life: 40 } as HouseRules | undefined],
    ['no rules', undefined],
  ])('says nothing for a game with %s', async (_what, rules) => {
    const h = await openPrecons({ rules })
    await h.settle()
    expect(document.querySelector('[data-slot="lobby-rules"]')).toBeNull()
  })
})
