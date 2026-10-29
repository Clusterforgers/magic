// Goal: the fix-mark button is a toggle that shows the account's mark as pressed, marks and unmarks at once before the server answers, puts the state back and says why when the server refuses, takes an optional note in a popover, ignores a press while one is on its way, and is not shown to a visitor without a session or on a server without a card database.
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { DeckError } from '@/net/decks'
import type { FixMarksApi } from '@/net/fixMarks'
import type { FixMarkView } from '@/protocol/generated'
import { fixMarksClient } from './client'
import { FixMarkButton } from './FixMarkButton'

const ORACLE = 'oracle-sol-ring'
const view = (note: string | null = null, oracleId = ORACLE): FixMarkView => ({
  oracle_id: oracleId,
  name: 'Sol Ring',
  note,
  created_at: '2026-10-29T12:00:00Z',
})

/** A fix-mark server in memory whose calls are spies; `mark` and `unmark` can be held until the test lets them go. */
function fakeApi(initial: FixMarkView[] = []) {
  const marks = [...initial]
  const api = {
    list: vi.fn(async () => [...marks]),
    mark: vi.fn(async (oracleId: string, note?: string | null) => view(note ?? null, oracleId)),
    unmark: vi.fn(async (_oracleId: string) => undefined),
  }
  return api satisfies FixMarksApi
}

const toggle = () => screen.getByRole('button', { name: 'Mark important to fix' })

/** Resolves once the list has loaded, which the button shows by taking presses. */
async function loaded() {
  await waitFor(() => expect(toggle()).toHaveAttribute('aria-disabled', 'false'))
}

function mount(api: FixMarksApi, emphasis?: 'prominent' | 'quiet') {
  return render(<FixMarkButton oracleId={ORACLE} name="Sol Ring" api={api} emphasis={emphasis} />)
}

beforeEach(() => {
  fixMarksClient.clear()
})

describe('FixMarkButton', () => {
  it('is an unpressed toggle for a card the account has not marked', async () => {
    mount(fakeApi())
    await loaded()
    expect(toggle()).toHaveAttribute('aria-pressed', 'false')
  })

  it('is pressed for a card the account has marked, and shows its note', async () => {
    mount(fakeApi([view('adds the wrong mana'), view('another card', 'oracle-other')]))
    await waitFor(() => expect(toggle()).toHaveAttribute('aria-pressed', 'true'))
    expect(screen.getByText('Your note: adds the wrong mana')).toBeInTheDocument()
  })

  it('marks the card before the server answers, then keeps the server’s mark', async () => {
    const user = userEvent.setup()
    const api = fakeApi()
    let release: (mark: FixMarkView) => void = () => undefined
    api.mark.mockImplementation(
      () =>
        new Promise<FixMarkView>((resolve) => {
          release = resolve
        }),
    )
    mount(api)
    await loaded()

    await user.click(toggle())
    expect(toggle()).toHaveAttribute('aria-pressed', 'true')
    expect(api.mark).toHaveBeenCalledExactlyOnceWith(ORACLE, null)

    await act(async () => release(view()))
    expect(toggle()).toHaveAttribute('aria-pressed', 'true')
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('takes the mark back at once and tells the server', async () => {
    const user = userEvent.setup()
    const api = fakeApi([view()])
    mount(api)
    await waitFor(() => expect(toggle()).toHaveAttribute('aria-pressed', 'true'))
    await loaded()

    await user.click(toggle())

    expect(toggle()).toHaveAttribute('aria-pressed', 'false')
    expect(api.unmark).toHaveBeenCalledExactlyOnceWith(ORACLE)
  })

  it('puts the state back and says why when the server refuses a mark', async () => {
    const user = userEvent.setup()
    const api = fakeApi()
    api.mark.mockRejectedValue(new DeckError('internal', 500, 'boom'))
    mount(api)
    await loaded()

    await user.click(toggle())

    await waitFor(() => expect(toggle()).toHaveAttribute('aria-pressed', 'false'))
    expect(await screen.findByRole('alert')).toHaveTextContent('The server ran into a problem')
  })

  it('puts the mark back when the server refuses to take it away, and a new press clears the message', async () => {
    const user = userEvent.setup()
    const api = fakeApi([view()])
    api.unmark.mockRejectedValueOnce(new DeckError('network', 0, 'offline'))
    mount(api)
    await waitFor(() => expect(toggle()).toHaveAttribute('aria-pressed', 'true'))
    await loaded()

    await user.click(toggle())
    expect(await screen.findByRole('alert')).toHaveTextContent('Could not reach the server')
    await waitFor(() => expect(toggle()).toHaveAttribute('aria-pressed', 'true'))

    await user.click(toggle())
    await waitFor(() => expect(toggle()).toHaveAttribute('aria-pressed', 'false'))
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('ignores a second press while the first is on its way', async () => {
    const user = userEvent.setup()
    const api = fakeApi()
    api.mark.mockImplementation(() => new Promise<FixMarkView>(() => undefined))
    mount(api)
    await loaded()

    await user.click(toggle())
    await user.click(toggle())

    expect(api.mark).toHaveBeenCalledTimes(1)
    expect(api.unmark).not.toHaveBeenCalled()
    expect(toggle()).toHaveAttribute('aria-pressed', 'true')
  })

  it('takes an optional note of a marked card in a popover and shows it', async () => {
    const user = userEvent.setup()
    const api = fakeApi()
    mount(api)
    await loaded()
    expect(screen.queryByRole('button', { name: 'Add note' })).toBeNull()

    await user.click(toggle())
    await user.click(await screen.findByRole('button', { name: 'Add note' }))
    const field = await screen.findByRole('textbox')
    await user.type(field, '  adds the wrong mana ')
    expect(screen.getByText('22 of 280')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Save note' }))

    expect(api.mark).toHaveBeenLastCalledWith(ORACLE, 'adds the wrong mana')
    expect(await screen.findByText('Your note: adds the wrong mana')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Edit note' })).toBeInTheDocument()
  })

  it('offers no more than 280 characters of note', async () => {
    const user = userEvent.setup()
    mount(fakeApi([view('old')]))
    await user.click(await screen.findByRole('button', { name: 'Edit note' }))
    const field = await screen.findByRole('textbox')
    expect(field).toHaveValue('old')
    expect(field).toHaveAttribute('maxlength', '280')
  })

  it.each([
    ['unauthenticated', 401],
    ['cards_unavailable', 503],
  ] as const)('is not shown when the list is refused with %s', async (code, status) => {
    const api = fakeApi()
    api.list.mockRejectedValue(new DeckError(code, status, 'no'))
    mount(api)
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Mark important to fix' })).toBeNull())
  })

  it('still works when the list could not be read for another reason', async () => {
    const user = userEvent.setup()
    const api = fakeApi()
    api.list.mockRejectedValue(new DeckError('network', 0, 'offline'))
    mount(api)
    await loaded()

    await user.click(toggle())

    expect(toggle()).toHaveAttribute('aria-pressed', 'true')
    expect(api.mark).toHaveBeenCalledTimes(1)
  })

  it('looks quieter for a card the game plays whole and keeps the same name', async () => {
    mount(fakeApi(), 'quiet')
    await loaded()
    expect(toggle().className).toContain('text-muted-foreground')
  })
})
