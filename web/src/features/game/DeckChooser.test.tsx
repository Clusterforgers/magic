// Goal: the deck chooser is a master-detail: rows with a summary each, search and filters, a selection that previews the deck (commanders, legality at the table, support, its cards) and is taken by "Use this deck", Enter or a double click, with arrow keys through the list, designed loading, empty and failed states, the preview beside the list on a wide page and in a dialog on a narrow one.
import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { renderInRouter } from '@/test-utils/router'
import { deckFacts, lobbyDecks, lobbyPrecons } from './dev/deckApis'
import type { DevDeckSwitches } from './dev/deckApis'
import { DeckChooser } from './DeckChooser'
import type { DeckChoice } from './useDeckChoice'

afterEach(() => vi.unstubAllGlobals())

function width(wide: boolean): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: wide,
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

interface Scene {
  format?: 'normal' | 'commander'
  switches?: DevDeckSwitches
  signedIn?: boolean
  ready?: boolean
  current?: Parameters<typeof DeckChooser>[0]['current']
  chosenId?: string | null
}

async function open({
  format = 'commander',
  switches = {},
  signedIn = true,
  ready = false,
  current = null,
  chosenId = null,
}: Scene = {}) {
  const choose = vi.fn()
  const choice: DeckChoice = { pending: null, chosenId, note: null, problem: null, choose }
  const auth = createAuthStore({
    api: { ...offlineAccounts, myGames: async () => [] },
    initial: signedIn
      ? { status: 'signed-in', user: { id: 'u', username: 'Ada_L' } }
      : { status: 'unavailable' },
  })
  await renderInRouter(
    <AuthContext value={auth}>
      <DeckChooser
        api={{ listDecks: async () => [] }}
        decks={lobbyDecks(switches)}
        precons={lobbyPrecons(switches)}
        game="g1"
        format={format}
        signedIn={signedIn}
        current={current}
        ready={ready}
        busy={false}
        choice={choice}
      />
    </AuthContext>,
  )
  return { choose, user: userEvent.setup() }
}

// The rows of the list: the support filter's own options are options too.
const deckRows = (): HTMLElement[] =>
  screen.queryAllByRole('option').filter((row) => row.hasAttribute('data-deck-option'))
const option = (name: string | RegExp): HTMLElement => screen.getByRole('option', { name })
const pane = (): HTMLElement => screen.getByText((_, node) => node?.getAttribute('data-slot') === 'deck-pane')

describe('the rows', () => {
  it.each([
    ['Burn', /Modern · 60 cards/, '3 played by hand'],
    ['Pauper Affinity', /Pauper · 60 cards/, 'Cannot be taken'],
    ['Rakdos Midrange', /Standard · 60 cards/, 'Whole deck runs'],
  ])('summarises %s with its facts and a support chip', async (name, facts, chip) => {
    width(true)
    await open({ format: 'normal' })
    const row = await screen.findByRole('option', { name: new RegExp(`^${name}`) })
    expect(within(row).getByText(facts)).toBeInTheDocument()
    expect(within(row).getByText(chip)).toBeInTheDocument()
  })

  it('shows a Commander game only its commander decks, with the commander and colours, and says how many are left out', async () => {
    width(true)
    await open()
    const row = await screen.findByRole('option', { name: /Atraxa Superfriends/ })
    expect(row).toHaveTextContent(/Atraxa, Praetors’ Voice/)
    expect(
      within(row).getByRole('img', { name: /Color identity: White, Blue, Black and Green/ }),
    ).toBeInTheDocument()
    expect(screen.queryByRole('option', { name: /^Burn/ })).toBeNull()
    expect(screen.getByText('3 decks are in other formats and not shown.')).toBeInTheDocument()
  })

  it('marks the deck the seat plays', async () => {
    width(true)
    await open({ format: 'normal', current: { name: 'Burn', format: 'modern', manual_cards: 1 } })
    const row = await screen.findByRole('option', { name: /^Burn/ })
    expect(row).toHaveAttribute('data-chosen', 'true')
    await waitFor(() => expect(row).toHaveAttribute('aria-selected', 'true'))
  })
})

describe('search and filters', () => {
  it.each([
    ['bur', ['Burn']],
    ['pauper', ['Pauper Affinity']],
    ['rakdos', ['Rakdos Midrange']],
    ['zzz', []],
  ])('narrows the list to the decks matching %j', async (query, names) => {
    width(true)
    const { user } = await open({ format: 'normal' })
    await screen.findByRole('option', { name: /^Burn/ })
    await user.type(screen.getByRole('searchbox'), query)
    expect(deckRows().map((row) => row.getAttribute('data-deck-option'))).toHaveLength(names.length)
    for (const name of names)
      expect(screen.getByRole('option', { name: new RegExp(name) })).toBeInTheDocument()
    if (names.length === 0) expect(screen.getByText('No deck matches.')).toBeInTheDocument()
  })

  it.each([
    ['pauper', 'Pauper Affinity'],
    ['zzz', null],
  ])(
    'moves a selection the search %j hides to the first row shown, or clears it',
    async (query, selected) => {
      width(true)
      const { user } = await open({ format: 'normal' })
      await user.click(await screen.findByRole('option', { name: /^Burn/ }))
      await waitFor(() => expect(within(pane()).getByRole('heading', { name: 'Burn' })).toBeInTheDocument())
      await user.type(screen.getByRole('searchbox'), query)
      if (selected === null) {
        await waitFor(() => expect(within(pane()).queryByRole('heading', { name: 'Burn' })).toBeNull())
        expect(within(pane()).queryByRole('button', { name: 'Use this deck' })).toBeNull()
      } else {
        await waitFor(() =>
          expect(screen.getByRole('option', { name: new RegExp(selected) })).toHaveAttribute(
            'aria-selected',
            'true',
          ),
        )
        expect(await within(pane()).findByRole('heading', { name: selected })).toBeInTheDocument()
      }
    },
  )

  it.each([
    ['Fully validated', ['Rakdos Midrange']],
    ['Has hand-played cards', ['Burn']],
    ['All decks', ['Burn', 'Pauper Affinity', 'Rakdos Midrange']],
  ])('filters by support: %s', async (label, names) => {
    width(true)
    const { user } = await open({ format: 'normal' })
    await screen.findByRole('option', { name: /^Burn/ })
    await user.selectOptions(screen.getByRole('combobox', { name: 'Support' }), label)
    expect(deckRows().map((row) => row.textContent ?? '')).toHaveLength(names.length)
    for (const name of names)
      expect(screen.getByRole('option', { name: new RegExp(`^${name}`) })).toBeInTheDocument()
  })

  it('filters by color identity and clears the filters', async () => {
    width(true)
    const { user } = await open({ switches: { decks: 'many' } })
    await screen.findByRole('option', { name: /Atraxa Superfriends/ })
    const all = deckRows().length
    await user.click(screen.getByRole('button', { name: 'Red' }))
    const red = deckRows().length
    expect(red).toBeLessThan(all)
    expect(red).toBeGreaterThan(0)
    await user.click(screen.getByRole('button', { name: /Clear filters/ }))
    expect(deckRows()).toHaveLength(all)
  })
})

describe('selecting and choosing', () => {
  it('previews the selected deck: its name, legality at the table, support and cards by type', async () => {
    width(true)
    const { user } = await open()
    await user.click(await screen.findByRole('option', { name: /Atraxa Superfriends/ }))
    const preview = await waitFor(() => {
      const found = pane().querySelector('[data-slot="deck-preview"]')
      expect(found).not.toBeNull()
      return found as HTMLElement
    })
    expect(within(preview).getByRole('heading', { name: 'Atraxa Superfriends' })).toBeInTheDocument()
    expect(await within(preview).findByText('Legal in Commander')).toBeInTheDocument()
    expect(within(preview).getByRole('region', { name: 'Creatures' })).toBeInTheDocument()
    expect(within(preview).getByRole('list', { name: 'Commanders' })).toBeInTheDocument()
    expect(within(preview).getByRole('list', { name: 'Spells by mana value' })).toBeInTheDocument()
  })

  it('takes the selected deck with the button', async () => {
    width(true)
    const { user, choose } = await open()
    await user.click(await screen.findByRole('option', { name: /Atraxa Superfriends/ }))
    await user.click(await screen.findByRole('button', { name: 'Use this deck' }))
    expect(choose).toHaveBeenCalledWith({ id: 'stored-atraxa', name: 'Atraxa Superfriends' })
  })

  it('takes a deck with a double click and with Enter, and moves the selection with the arrow keys', async () => {
    width(true)
    const { user, choose } = await open({ format: 'normal' })
    const first = await screen.findByRole('option', { name: /^Burn/ })
    await waitFor(() => expect(first).toHaveAttribute('aria-selected', 'true'))
    first.focus()
    await user.keyboard('{ArrowDown}')
    expect(option(/^Pauper Affinity/)).toHaveAttribute('aria-selected', 'true')
    expect(option(/^Pauper Affinity/)).toHaveFocus()
    await user.keyboard('{End}')
    expect(option(/^Rakdos/)).toHaveAttribute('aria-selected', 'true')
    await user.keyboard('{ArrowDown}')
    expect(option(/^Rakdos/)).toHaveAttribute('aria-selected', 'true')
    await user.keyboard('{Home}{Enter}')
    expect(choose).toHaveBeenLastCalledWith({ id: 'stored-burn', name: 'Burn' })
    await user.dblClick(option(/^Rakdos/))
    expect(choose).toHaveBeenLastCalledWith({ id: 'stored-rakdos', name: 'Rakdos Midrange' })
  })

  it('shows a deck the table cannot take with the reason and does not take it', async () => {
    width(true)
    const { user, choose } = await open({ format: 'normal' })
    await user.click(await screen.findByRole('option', { name: /^Pauper Affinity/ }))
    const why = await screen.findByText('Not legal in Normal')
    expect(why).toBeInTheDocument()
    const use = screen.getByRole('button', { name: 'Use this deck' })
    await waitFor(() => expect(use).toHaveAttribute('aria-disabled', 'true'))
    await user.click(use)
    expect(choose).not.toHaveBeenCalled()
    expect(document.querySelector('[data-slot="deck-blocked"]')).toBeNull()
    expect(screen.getByRole('list', { name: /Why the deck is not legal in Normal/ })).toHaveTextContent(
      /not legal in this format/i,
    )
  })

  it('lists the cards of a built-in deck by name', async () => {
    width(true)
    const { user } = await open({ format: 'normal', signedIn: false })
    await user.click(await screen.findByRole('tab', { name: 'Precons' }))
    expect(await screen.findByRole('option', { name: /Mono White Starter/ })).toBeInTheDocument()
  })

  it('previews a precon with its cards and a way to read the cards played by hand', async () => {
    width(true)
    const { user } = await open({ signedIn: false })
    await user.click(await screen.findByRole('option', { name: /Merciless Rage/ }))
    const link = await screen.findByRole('button', { name: 'Not validated cards' })
    await user.click(link)
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText('Cyclonic Rift')).toBeInTheDocument()
  })
})

describe('states', () => {
  it('shows skeleton rows while the list loads', async () => {
    width(true)
    await open({ switches: { list: 'loading' } })
    await waitFor(() =>
      expect(document.querySelector('[data-slot="decks-loading"]')).toHaveAttribute('aria-busy', 'true'),
    )
    expect(document.querySelector('[data-slot="decks-loading"]')).not.toBeNull()
  })

  it('leads an account without decks to the deck builder', async () => {
    width(true)
    await open({ switches: { list: 'empty' } })
    expect(await screen.findByRole('heading', { name: 'You have no saved decks yet' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Build or import a deck' })).toHaveAttribute('href', '/decks')
  })

  it('says why the list is missing and offers another try', async () => {
    width(true)
    const { user } = await open({ switches: { list: 'failed' } })
    expect(await screen.findByText(/./, { selector: '[data-slot="decks-failed"]' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(document.querySelector('[data-slot="decks-failed"]')).not.toBeNull()
  })

  it('says why the cards of a deck are missing and tries again', async () => {
    width(true)
    const { user } = await open({ switches: { detail: 'failed' } })
    await user.click(await screen.findByRole('option', { name: /Atraxa Superfriends/ }))
    expect(await screen.findByText(/./, { selector: '[data-slot="deck-pane-failed"] p' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Use this deck' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Try again' }))
  })

  it('locks the deck while the seat is ready and keeps the list away', async () => {
    width(true)
    await open({
      ready: true,
      format: 'normal',
      current: { name: 'Burn', format: 'modern', manual_cards: 1 },
    })
    expect(
      screen.getByText('Your deck is locked while you are ready. Press Not ready to change it.'),
    ).toBeInTheDocument()
    expect(screen.queryByRole('listbox')).toBeNull()
    expect(deckFacts('stored-burn')).toBeDefined()
  })
})

describe('one column and two', () => {
  it('draws the preview beside the list on a wide page', async () => {
    width(true)
    await open()
    await screen.findByRole('option', { name: /Atraxa Superfriends/ })
    expect(document.querySelector('[data-slot="deck-pane"]')).not.toBeNull()
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('opens the preview as a dialog from a row on a narrow page, and takes the deck from it', async () => {
    width(false)
    const { user, choose } = await open()
    expect(document.querySelector('[data-slot="deck-pane"]')).toBeNull()
    await user.click(await screen.findByRole('option', { name: /Atraxa Superfriends/ }))
    const sheet = await screen.findByRole('dialog')
    expect(within(sheet).getByRole('heading', { name: 'Atraxa Superfriends' })).toBeInTheDocument()
    await user.click(await within(sheet).findByRole('button', { name: 'Use this deck' }))
    expect(choose).toHaveBeenCalledWith({ id: 'stored-atraxa', name: 'Atraxa Superfriends' })
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  })
})
