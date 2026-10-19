// Goal: Import deck reads a pasted list or a chosen file in two steps: it refuses an empty, over-long or oversized list before sending, sends the text and the chosen format and lets the server detect the kind of list, then shows what was read together with the ignored parts, the lines no card was found for and the lines that were no entries, and saves the resolved cards under a name only after the review; failures stay in the dialog with their reason.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { deckError, deckItem, importResult, lineOf } from './fakeDecks'
import { MAX_IMPORT_FILE_BYTES, MAX_IMPORT_LINES } from './rules'
import { deckRows, listed, renderDecks, requests } from './renderDecks'

type App = Awaited<ReturnType<typeof renderDecks>>

const dialog = () => screen.queryByRole('dialog')
const listField = () => screen.getByLabelText('Deck list')
const readButton = () => screen.getByRole('button', { name: 'Read list' })
const nameField = () => screen.getByLabelText('Deck name')
const LIST = '4 Lightning Bolt\n20 Mountain'

async function openImport(script: Parameters<typeof renderDecks>[0] = {}) {
  const app = await renderDecks({ decks: [deckItem({ name: 'Atraxa' })], ...script })
  await listed()
  // An empty list has a second "Import deck" in its empty state; the first is the toolbar's.
  await app.user.click(screen.getAllByRole('button', { name: 'Import deck' })[0] as HTMLElement)
  const form = await screen.findByRole('dialog', { name: 'Import a deck' })
  return { ...app, form }
}

async function paste(app: App, text: string) {
  await app.user.click(listField())
  await app.user.paste(text)
}

/** Pastes `list`, reads it, and resolves in the review step. */
async function review(app: App, list = LIST) {
  await paste(app, list)
  await app.user.click(readButton())
  return screen.findByRole('dialog', { name: 'Review the list' })
}

describe('the first step', () => {
  it('opens on the list, with Commander chosen and a way to choose a file', async () => {
    const { form } = await openImport()
    await waitFor(() => expect(listField()).toHaveFocus())
    expect(within(form).getByLabelText('Format')).toHaveValue('commander')
    expect(within(form).getByLabelText('Or choose a file')).toHaveAttribute('type', 'file')
    expect(within(form).getByText(/the server detects which kind it is/i)).toBeInTheDocument()
  })

  it('sends nothing on Cancel and on Escape', async () => {
    const { user, form, server } = await openImport()
    await user.click(within(form).getByRole('button', { name: 'Cancel' }))
    await waitFor(() => expect(dialog()).toBeNull())
    await user.click(screen.getAllByRole('button', { name: 'Import deck' })[0] as HTMLElement)
    await screen.findByRole('dialog', { name: 'Import a deck' })
    await user.keyboard('{Escape}')
    await waitFor(() => expect(dialog()).toBeNull())
    expect(requests(server.calls)).toEqual(['GET /api/decks'])
  })

  it.each([
    ['nothing', '', /paste a deck list or choose a file/i],
    ['only blank lines', '\n\n  \n', /paste a deck list or choose a file/i],
    [
      'more lines than the server reads',
      Array.from({ length: MAX_IMPORT_LINES + 1 }, () => '1 Island').join('\n'),
      /at most 1,000 lines \(this one has 1,001\)/,
    ],
    ['more characters than the server reads', 'x'.repeat(100_001), /at most 100,000 characters/],
  ])('refuses %s with words, sends nothing, and puts the focus on the list', async (_what, text, words) => {
    const app = await openImport()
    if (text !== '') await paste(app, text)
    await app.user.click(readButton())
    expect(await screen.findByText(words)).toBeInTheDocument()
    expect(listField()).toHaveAttribute('aria-invalid', 'true')
    expect(listField()).toHaveAccessibleDescription(words)
    expect(listField()).toHaveFocus()
    expect(app.server.imports).toEqual([])
  })

  it('sends the pasted text as it is with the chosen format and no kind, so the server detects it', async () => {
    const app = await openImport()
    await app.user.selectOptions(screen.getByLabelText('Format'), 'modern')
    await review(app)
    expect(app.server.imports).toEqual([{ text: LIST, source: null, format: 'modern' }])
    expect(app.server.calls.at(-1)?.url).toBe('/api/decks/import')
    expect(app.server.calls.at(-1)?.init?.credentials).toBe('include')
  })

  it('reads a chosen file into the list and suggests its name for the deck', async () => {
    const app = await openImport()
    const file = new File(['4 Lightning Bolt\n'], 'Burn Deck.txt', { type: 'text/plain' })
    await app.user.upload(screen.getByLabelText('Or choose a file'), file)
    await waitFor(() => expect(listField()).toHaveValue('4 Lightning Bolt\n'))
    await app.user.click(readButton())
    await screen.findByRole('dialog', { name: 'Review the list' })
    expect(nameField()).toHaveValue('Burn Deck')
    expect(app.server.imports[0]?.text).toBe('4 Lightning Bolt\n')
  })

  it('refuses a file larger than the server reads and leaves the list as it was', async () => {
    const app = await openImport()
    await paste(app, LIST)
    const big = new File(['x'], 'big.txt', { type: 'text/plain' })
    Object.defineProperty(big, 'size', { value: MAX_IMPORT_FILE_BYTES + 1 })
    await app.user.upload(screen.getByLabelText('Or choose a file'), big)
    expect(await screen.findByText(/too big for a deck list/i)).toBeInTheDocument()
    expect(listField()).toHaveValue(LIST)
  })

  it('keeps the pasted text and says why when the server cannot read the request', async () => {
    const app = await openImport({
      override: { import: () => deckError(400, 'invalid_request', 'the list is at most 1000 lines') },
    })
    await paste(app, LIST)
    await app.user.click(readButton())
    expect(await within(app.form).findByRole('alert')).toHaveTextContent('The list is at most 1000 lines.')
    expect(listField()).toHaveValue(LIST)
    expect(readButton()).not.toHaveAttribute('aria-busy')
  })

  it('sends once when the button is pressed twice while the server reads', async () => {
    const release: { current: (() => void) | null } = { current: null }
    const app = await openImport({
      override: {
        import: () =>
          new Promise<Response>((resolve) => {
            release.current = () => resolve(deckError(500, 'internal', 'boom'))
          }),
      },
    })
    await paste(app, LIST)
    await app.user.click(readButton())
    await app.user.click(readButton())
    expect(app.server.imports).toHaveLength(1)
    expect(readButton()).toHaveAttribute('aria-busy', 'true')
    release.current?.()
    await screen.findByRole('alert')
  })
})

describe('the review', () => {
  it('says what was read and asks for a name, on the name', async () => {
    const app = await openImport({ importResult: importResult() })
    const form = await review(app)
    expect(within(form).getByText('Read as Arena: 24 cards.')).toBeInTheDocument()
    await waitFor(() => expect(nameField()).toHaveFocus())
    expect(within(form).getByText('Format: Commander. Go back to change it.')).toBeInTheDocument()
    expect(within(form).queryByRole('status')).toBeNull()
  })

  it('shows the parts of the list that are not deck cards, with their names and sizes', async () => {
    const app = await openImport({
      importResult: importResult({
        source: 'moxfield',
        ignored: [
          { name: 'Maybeboard', count: 12 },
          { name: 'Stickers', count: 1 },
        ],
      }),
    })
    const form = await review(app)
    const note = within(form)
      .getByText('Left out of the deck')
      .closest('[data-note="ignored"]') as HTMLElement
    expect(note).toHaveAttribute('role', 'status')
    expect(note).toHaveTextContent('Maybeboard (12 cards) is not part of the deck.')
    expect(note).toHaveTextContent('Stickers (1 card) is not part of the deck.')
    expect(within(form).getByText('Read as Moxfield: 24 cards.')).toBeInTheDocument()
  })

  it('lists the lines no card was found for, with the names that might have been meant, and the lines that were no entries', async () => {
    const app = await openImport({
      importResult: importResult({
        unresolved: [
          {
            name: 'Lightening Bolt',
            count: 2,
            zone: 'main',
            reason: 'unknown_card',
            message: 'no card has this name',
            suggestions: [
              { oracle_id: 'a', name: 'Lightning Bolt' },
              { oracle_id: 'b', name: 'Lightning Helix' },
            ],
          },
          {
            name: 'Bolt',
            count: 1,
            zone: 'sideboard',
            reason: 'ambiguous_name',
            message: 'several cards match',
            suggestions: [],
          },
        ],
        problems: [{ line: 7, message: 'no count' }],
      }),
    })
    const form = await review(app)
    const missing = within(form)
      .getByText(/2 lines were not found and are left out of the deck/)
      .closest('[data-note]') as HTMLElement
    expect(missing).toHaveTextContent(
      '2 Lightening Bolt: no card has this name Did you mean Lightning Bolt, Lightning Helix?',
    )
    expect(missing).toHaveTextContent('1 Bolt: several cards match')
    expect(missing).not.toHaveTextContent('Did you mean Bolt')
    const problems = within(form).getByText('1 line is no card entry').closest('[data-note]') as HTMLElement
    expect(problems).toHaveTextContent('Line 7: no count')
  })

  it('says "1 line was" for a single line', async () => {
    const app = await openImport({
      importResult: importResult({
        unresolved: [
          {
            name: 'Foo',
            count: 1,
            zone: 'main',
            reason: 'unknown_card',
            message: 'no such card',
            suggestions: [],
          },
        ],
      }),
    })
    const form = await review(app)
    expect(within(form).getByText('1 line was not found and is left out of the deck')).toBeInTheDocument()
  })

  it('goes back to the list with the text and the format kept', async () => {
    const app = await openImport()
    await app.user.selectOptions(screen.getByLabelText('Format'), 'pauper')
    const form = await review(app)
    await app.user.click(within(form).getByRole('button', { name: 'Back' }))
    await screen.findByRole('dialog', { name: 'Import a deck' })
    expect(listField()).toHaveValue(LIST)
    expect(screen.getByLabelText('Format')).toHaveValue('pauper')
  })

  it('has nothing to save when no card could be read, and says so', async () => {
    const app = await openImport({ importResult: importResult({ cards: [] }) })
    const form = await review(app)
    expect(within(form).getByText(/no cards could be read from this list/i)).toBeInTheDocument()
    expect(within(form).queryByRole('button', { name: 'Save deck' })).toBeNull()
    expect(within(form).queryByLabelText('Deck name')).toBeNull()
    expect(within(form).getByRole('button', { name: 'Back' })).toBeInTheDocument()
  })
})

describe('saving', () => {
  it('needs a name the server would take, and sends nothing without one', async () => {
    const app = await openImport()
    const form = await review(app)
    await app.user.click(within(form).getByRole('button', { name: 'Save deck' }))
    expect(await within(form).findByText(/enter a name for the deck/i)).toBeInTheDocument()
    expect(nameField()).toHaveAttribute('aria-invalid', 'true')
    expect(nameField()).toHaveFocus()
    expect(app.server.created).toEqual([])
  })

  it('saves the resolved cards zone by zone, with their printings, under the trimmed name, then lists the deck first', async () => {
    const app = await openImport({
      importResult: importResult({
        cards: [
          lineOf('Lightning Bolt', 4),
          lineOf('Mountain', 20, { printing_id: 'printing-1' }),
          lineOf('Duress', 2, { zone: 'sideboard' }),
          lineOf('Tormod, the Desecrator', 1, { zone: 'commander' }),
        ],
      }),
    })
    const form = await review(app)
    await app.user.type(nameField(), '  Imported Burn  ')
    await app.user.click(within(form).getByRole('button', { name: 'Save deck' }))
    await waitFor(() => expect(dialog()).toBeNull())
    expect(app.server.created).toEqual([
      {
        name: 'Imported Burn',
        format: 'commander',
        notes: '',
        cards: [
          { zone: 'main', oracle_id: 'oracle-lightning-bolt', printing_id: null, count: 4 },
          { zone: 'main', oracle_id: 'oracle-mountain', printing_id: 'printing-1', count: 20 },
          { zone: 'sideboard', oracle_id: 'oracle-duress', printing_id: null, count: 2 },
          { zone: 'commander', oracle_id: 'oracle-tormod-the-desecrator', printing_id: null, count: 1 },
        ],
      },
    ])
    await waitFor(() => expect(deckRows()).toHaveLength(2))
    expect(deckRows()[0]).toHaveTextContent('Imported Burn')
    expect(deckRows()[0]).toHaveTextContent('Commander · 27 cards')
    expect(deckRows()[0]).toHaveTextContent('Verified')
    expect(await screen.findByText('Saved “Imported Burn”: Verified.')).toBeInTheDocument()
    expect(requests(app.server.calls)).toEqual([
      'GET /api/decks',
      'POST /api/decks/import',
      'POST /api/decks',
      'GET /api/decks',
    ])
  })

  it('saves in the format chosen for the reading', async () => {
    const app = await openImport()
    await app.user.selectOptions(screen.getByLabelText('Format'), 'legacy')
    await review(app)
    await app.user.type(nameField(), 'Legacy Burn{Enter}')
    await waitFor(() => expect(app.server.created[0]?.format).toBe('legacy'))
    await waitFor(() => expect(dialog()).toBeNull())
    expect(deckRows()[0]).toHaveTextContent('Legacy')
  })

  it('does not replace a name the user typed with the file name when the list is read again', async () => {
    const app = await openImport()
    const file = new File(['4 Lightning Bolt\n'], 'Burn Deck.txt', { type: 'text/plain' })
    await app.user.upload(screen.getByLabelText('Or choose a file'), file)
    await waitFor(() => expect(listField()).toHaveValue('4 Lightning Bolt\n'))
    await app.user.click(readButton())
    const form = await screen.findByRole('dialog', { name: 'Review the list' })
    await app.user.clear(nameField())
    await app.user.type(nameField(), 'Mine')
    await app.user.click(within(form).getByRole('button', { name: 'Back' }))
    await app.user.click(await screen.findByRole('button', { name: 'Read list' }))
    await screen.findByRole('dialog', { name: 'Review the list' })
    expect(nameField()).toHaveValue('Mine')
  })

  it.each([
    ['the deck limit', 409, 'deck_limit', /delete one to make room/i],
    ['cards the database does not know', 400, 'unknown_cards', /not in the server's card database/i],
    [
      'too many lines for a deck',
      400,
      'invalid_deck',
      /did not accept the deck. A deck has at most 250 lines\./,
    ],
  ] as const)(
    'keeps the review up with %s and lets the user try again',
    async (_what, status, code, words) => {
      let attempts = 0
      const app = await openImport({
        override: {
          create: () => {
            attempts += 1
            return attempts === 1 ? deckError(status, code, 'a deck has at most 250 lines') : undefined
          },
        },
      })
      const form = await review(app)
      await app.user.type(nameField(), 'Burn')
      await app.user.click(within(form).getByRole('button', { name: 'Save deck' }))
      expect(await within(form).findByRole('alert')).toHaveTextContent(words)
      expect(nameField()).toHaveValue('Burn')
      await app.user.click(within(form).getByRole('button', { name: 'Save deck' }))
      await waitFor(() => expect(dialog()).toBeNull())
      expect(app.server.created).toHaveLength(2)
    },
  )
})
