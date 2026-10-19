// Goal: New deck asks for a name and a format, refuses a name the server would refuse before sending anything, saves a deck without cards under the trimmed name and the chosen format and then lists it first, puts the server's reason in the dialog when a save fails, and sends nothing on Cancel or Escape.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { DECK_FORMATS, formatLabel } from './format'
import { deckError, deckItem } from './fakeDecks'
import { MAX_DECK_NAME_CHARS } from './rules'
import { deckRows, listed, renderDecks, requests } from './renderDecks'

const dialog = () => screen.queryByRole('dialog')
const nameField = () => screen.getByLabelText('Deck name')
const createButton = () => screen.getByRole('button', { name: 'Create deck' })

async function openNew(
  script: Parameters<typeof renderDecks>[0] = { decks: [deckItem({ name: 'Atraxa' })] },
) {
  const app = await renderDecks(script)
  await listed()
  // An empty list has a second "New deck" in its empty state; the first is the toolbar's.
  await app.user.click(screen.getAllByRole('button', { name: 'New deck' })[0] as HTMLElement)
  const form = await screen.findByRole('dialog', { name: 'New deck' })
  return { ...app, form }
}

describe('the dialog', () => {
  it('opens on the name, with Commander chosen and every format the server checks on offer', async () => {
    const { form } = await openNew()
    await waitFor(() => expect(nameField()).toHaveFocus())
    expect(within(form).getByLabelText('Format')).toHaveValue('commander')
    const options = within(within(form).getByLabelText('Format')).getAllByRole('option')
    expect(options.map((option) => option.textContent)).toEqual(DECK_FORMATS.map(formatLabel))
  })

  it('sends nothing on Cancel and on Escape', async () => {
    const { user, form, server } = await openNew()
    await user.click(within(form).getByRole('button', { name: 'Cancel' }))
    await waitFor(() => expect(dialog()).toBeNull())
    await user.click(screen.getByRole('button', { name: 'New deck' }))
    await screen.findByRole('dialog', { name: 'New deck' })
    await user.keyboard('{Escape}')
    await waitFor(() => expect(dialog()).toBeNull())
    expect(requests(server.calls)).toEqual(['GET /api/decks'])
    await waitFor(() => expect(screen.getByRole('button', { name: 'New deck' })).toHaveFocus())
  })

  it('starts empty each time it is opened', async () => {
    const { user } = await openNew()
    await user.type(nameField(), 'Draft')
    await user.keyboard('{Escape}')
    await waitFor(() => expect(dialog()).toBeNull())
    await user.click(screen.getByRole('button', { name: 'New deck' }))
    await screen.findByRole('dialog', { name: 'New deck' })
    expect(nameField()).toHaveValue('')
  })
})

describe('a name the server would refuse', () => {
  it.each([
    ['nothing', '', /enter a name/i],
    ['only spaces', '   ', /enter a name/i],
    [
      'too many characters',
      'n'.repeat(MAX_DECK_NAME_CHARS + 1),
      /at most 100 characters \(this one has 101\)/,
    ],
  ])('is refused with words when it is %s, and nothing is sent', async (_what, name, words) => {
    const { user, server } = await openNew()
    if (name !== '') await user.click(nameField()).then(() => user.paste(name))
    await user.click(createButton())
    expect(await screen.findByText(words)).toBeInTheDocument()
    expect(nameField()).toHaveAttribute('aria-invalid', 'true')
    expect(nameField()).toHaveAccessibleDescription(words)
    expect(nameField()).toHaveFocus()
    expect(server.created).toEqual([])
    expect(requests(server.calls)).toEqual(['GET /api/decks'])
  })

  it('loses its complaint as soon as it is edited', async () => {
    const { user } = await openNew()
    await user.click(createButton())
    await screen.findByText(/enter a name/i)
    await user.type(nameField(), 'B')
    expect(screen.queryByText(/enter a name/i)).toBeNull()
    expect(nameField()).toHaveAttribute('aria-invalid', 'false')
  })
})

describe('saving', () => {
  it('sends the trimmed name, the chosen format and no cards, then lists the new deck first', async () => {
    const { user, form, server } = await openNew()
    await user.type(nameField(), '  Burn  ')
    await user.selectOptions(within(form).getByLabelText('Format'), 'modern')
    await user.click(createButton())
    await waitFor(() => expect(dialog()).toBeNull())
    expect(server.created).toEqual([{ name: 'Burn', format: 'modern', notes: '', cards: [] }])
    expect(server.calls[1]?.init?.credentials).toBe('include')
    await waitFor(() => expect(deckRows()).toHaveLength(2))
    expect(deckRows()[0]).toHaveTextContent('Burn')
    expect(deckRows()[0]).toHaveTextContent('Modern')
    expect(deckRows()[0]).toHaveTextContent('Blocked')
    expect(deckRows()[1]).toHaveTextContent('Atraxa')
    expect(requests(server.calls)).toEqual(['GET /api/decks', 'POST /api/decks', 'GET /api/decks'])
  })

  it('says what was saved and how it checked, aloud', async () => {
    const { user } = await openNew()
    await user.type(nameField(), 'Burn')
    await user.click(createButton())
    expect(await screen.findByText('Saved “Burn”: Blocked.')).toBeInTheDocument()
  })

  it('is sent by Enter in the name field', async () => {
    const { user, server } = await openNew()
    await user.type(nameField(), 'Burn{Enter}')
    await waitFor(() => expect(server.created).toHaveLength(1))
  })

  it('saves once when the button is pressed twice while the first is in flight', async () => {
    const release: { current: (() => void) | null } = { current: null }
    const { user, server } = await openNew({
      decks: [],
      override: {
        create: () =>
          new Promise<Response>((resolve) => {
            release.current = () => resolve(deckError(500, 'internal', 'boom'))
          }),
      },
    })
    await user.type(nameField(), 'Burn')
    await user.click(createButton())
    await user.click(createButton())
    expect(server.created).toHaveLength(1)
    expect(createButton()).toHaveAttribute('aria-busy', 'true')
    release.current?.()
    await screen.findByRole('alert')
  })
})

describe('a save the server refuses', () => {
  it.each([
    ['the deck limit', 409, 'deck_limit', /delete one to make room/i],
    ['a name it dislikes', 400, 'invalid_deck', /did not accept the deck. The name is blank\./],
    ['an ended session', 401, 'unauthenticated', /session has ended/],
    ['a server that lost its database', 503, 'decks_unavailable', /keeps no decks/],
  ] as const)('puts %s in the dialog and keeps what was typed', async (_what, status, code, words) => {
    const { user, form } = await openNew({
      decks: [],
      override: { create: () => deckError(status, code, 'the name is blank') },
    })
    await user.type(nameField(), 'Burn')
    await user.click(createButton())
    expect(await within(form).findByRole('alert')).toHaveTextContent(words)
    expect(dialog()).toBeInTheDocument()
    expect(nameField()).toHaveValue('Burn')
    expect(createButton()).not.toHaveAttribute('aria-busy')
  })

  it('puts a lost connection in the dialog too, and a second try can succeed', async () => {
    let attempts = 0
    const { user, form, server } = await openNew({
      decks: [],
      override: {
        create: () => {
          attempts += 1
          return attempts === 1 ? Promise.reject(new TypeError('Failed to fetch')) : undefined
        },
      },
    })
    await user.type(nameField(), 'Burn')
    await user.click(createButton())
    expect(await within(form).findByRole('alert')).toHaveTextContent(/could not reach the server/i)
    await user.click(createButton())
    await waitFor(() => expect(dialog()).toBeNull())
    expect(server.created).toHaveLength(2)
    await waitFor(() => expect(deckRows()).toHaveLength(1))
  })
})
