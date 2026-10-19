// Goal: a Join on the Play page takes a seat and opens the game: a signed-in visitor in one click with no body and the seat bound to the account, a guest after a dialog that asks for a name (remembered, focus in and out, Escape and Cancel leave without sending), with one request at a time, the seat link in the fragment unless the account's session opens the seat, a seat that is gone said in words with the list read again, and every other failure leaving the form to be sent again.
import { fireEvent, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import type { AuthStatus } from '@/features/auth/store'
import type { AccountsApi } from '@/net/accounts'
import { LobbyError } from '@/net/lobbyErrors'
import type { Joined, LobbyErrorCode, UserView } from '@/protocol/generated'
import { fakeLobbies, lobbyRow } from '@/test-utils/lobbies'
import type { FakeLobbies } from '@/test-utils/lobbies'
import { renderInRouter } from '@/test-utils/router'
import { NOT_OPEN } from './joinErrors'
import { PlayPage } from './PlayPage'

beforeEach(() => localStorage.clear())
afterEach(() => {
  vi.restoreAllMocks()
  localStorage.clear()
})

const ADA: UserView = { id: 'u1', username: 'Ada_L' }
const BO = lobbyRow({ game: 'g1', host: 'Bo', filled: 2, total: 4 })

function accountStore(status: AuthStatus, me?: AccountsApi['me']) {
  return createAuthStore({
    // `Continue` asks for the user's games when someone is signed in; an empty list keeps this file about joining.
    api: { ...offlineAccounts, myGames: () => Promise.resolve([]), ...(me === undefined ? {} : { me }) },
    initial: { status, user: status === 'signed-in' ? ADA : null },
  })
}

async function open(
  lobbies: FakeLobbies = fakeLobbies([BO]),
  store: ReturnType<typeof accountStore> | null = null,
) {
  const page = <PlayPage api={lobbies.api} />
  await renderInRouter(
    store === null ? page : <AuthContext.Provider value={store}>{page}</AuthContext.Provider>,
  )
  await screen.findByRole('table')
  return { lobbies, user: userEvent.setup() }
}

const joinButton = () => screen.getByRole('button', { name: "Join Bo's game, 2 of 4 seats" })
const rowButton = () => document.querySelector<HTMLElement>('[data-join="g1"]')
const dialog = () => screen.findByRole('dialog', { name: "Join Bo's game" })
const nameField = () => screen.getByLabelText('Your name')
const sendButton = () => screen.getByRole('button', { name: 'Join game' })
const gone = () => waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
const stub = () => screen.getByTestId('game-stub')

/** Presses Join on Bo's row, answers the guest's question with `name`, and sends it. */
async function joinAs(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(joinButton())
  await dialog()
  await user.clear(nameField())
  if (name !== '') await user.type(nameField(), name)
  await user.click(sendButton())
}

describe('a guest', () => {
  it('is asked for a name in a dialog named after the host, with focus on the field and nothing sent yet', async () => {
    const { lobbies, user } = await open()
    await user.click(joinButton())
    const form = await dialog()
    await waitFor(() => expect(nameField()).toHaveFocus())
    expect(within(form).getByText('Choose the name the other players see at the table.')).toBeVisible()
    expect(nameField()).toHaveValue('')
    expect(lobbies.joinGame).not.toHaveBeenCalled()
  })

  it('joins under the trimmed name and opens the game with the seat link in the fragment', async () => {
    const { lobbies, user } = await open()
    await joinAs(user, '  Cy  ')
    await waitFor(() => expect(stub()).toHaveTextContent('g1 /game/g1#seat=1&token=tok-1'))
    expect(lobbies.joinGame).toHaveBeenCalledExactlyOnceWith('g1', { name: 'Cy' })
  })

  it('opens the seat the server gave, whichever it is', async () => {
    const lobbies = fakeLobbies([BO])
    lobbies.joinGame.mockResolvedValueOnce({ game: 'g1', seat: 3, token: 'abc123' })
    const { user } = await open(lobbies)
    await joinAs(user, 'Cy')
    await waitFor(() => expect(stub()).toHaveTextContent('g1 /game/g1#seat=3&token=abc123'))
  })

  it.each([
    ['nothing', '', 'Enter a name for this seat.'],
    ['only spaces', '   ', 'Enter a name for this seat.'],
    ['33 characters', 'x'.repeat(33), 'Use at most 32 characters (this one has 33).'],
  ])('is refused %s with a reason on the field, and sends nothing', async (_what, name, reason) => {
    const { lobbies, user } = await open()
    await user.click(joinButton())
    await dialog()
    fireEvent.change(nameField(), { target: { value: name } })
    await user.click(sendButton())
    expect(screen.getByText(reason)).toBeVisible()
    expect(nameField()).toBeInvalid()
    expect(nameField()).toHaveAccessibleDescription(reason)
    expect(nameField()).toHaveFocus()
    expect(lobbies.joinGame).not.toHaveBeenCalled()
    await user.type(nameField(), 'y')
    expect(screen.queryByText(reason)).not.toBeInTheDocument()
  })

  it('is refused a name with a control character', async () => {
    const { lobbies, user } = await open()
    await user.click(joinButton())
    await dialog()
    fireEvent.change(nameField(), { target: { value: 'Cy\u0007' } })
    await user.click(sendButton())
    expect(screen.getByText('Remove control or invisible characters from the name.')).toBeVisible()
    expect(lobbies.joinGame).not.toHaveBeenCalled()
  })

  it('sends one request at a time: the buttons wait, a second Enter sends nothing, Escape keeps the dialog', async () => {
    const lobbies = fakeLobbies([BO])
    let answer: (joined: Joined) => void = () => undefined
    lobbies.joinGame.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          answer = resolve
        }),
    )
    const { user } = await open(lobbies)
    await joinAs(user, 'Cy')
    await waitFor(() => expect(sendButton()).toHaveAttribute('aria-busy', 'true'))
    expect(rowButton()).toHaveAttribute('aria-disabled', 'true')
    expect(nameField()).toHaveAttribute('readonly')
    await user.keyboard('{Enter}')
    await user.keyboard('{Escape}')
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(lobbies.joinGame).toHaveBeenCalledTimes(1)
    answer({ game: 'g1', seat: 1, token: 'tok-1' })
    await waitFor(() => expect(stub()).toHaveTextContent('g1 /game/g1#seat=1&token=tok-1'))
  })

  it('has its name remembered for the next join, and a refusal does not lose it', async () => {
    const lobbies = fakeLobbies([BO])
    lobbies.joinGame.mockRejectedValueOnce(new LobbyError('unavailable', 503, 'busy'))
    const { user } = await open(lobbies)
    await joinAs(user, 'Cy')
    await screen.findByText('The server is busy or restarting. Wait a minute, then try again.')
    expect(localStorage.getItem('magic.guestName')).toBe('Cy')
    await user.click(screen.getByRole('button', { name: 'Cancel' }))
    await gone()
    await user.click(joinButton())
    await dialog()
    expect(nameField()).toHaveValue('Cy')
  })

  it('joins all the same when the browser keeps nothing', async () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('full', 'QuotaExceededError')
    })
    const { lobbies, user } = await open()
    await user.click(joinButton())
    await dialog()
    expect(nameField()).toHaveValue('')
    await user.type(nameField(), 'Cy')
    await user.click(sendButton())
    await waitFor(() => expect(stub()).toHaveTextContent('g1 /game/g1#seat=1&token=tok-1'))
    expect(lobbies.joinGame).toHaveBeenCalledExactlyOnceWith('g1', { name: 'Cy' })
  })

  it('is a guest also when nobody is signed in', async () => {
    const { user } = await open(fakeLobbies([BO]), accountStore('signed-out'))
    await user.click(joinButton())
    await dialog()
  })
})

describe('leaving the question', () => {
  const ways: [string, (user: ReturnType<typeof userEvent.setup>) => Promise<void>][] = [
    ['Cancel', (user) => user.click(screen.getByRole('button', { name: 'Cancel' }))],
    ['Escape', (user) => user.keyboard('{Escape}')],
    ['the close button', (user) => user.click(screen.getByRole('button', { name: 'Close' }))],
  ]

  it.each(ways)('by %s sends nothing and puts focus back on the Join button', async (_how, leave) => {
    const { lobbies, user } = await open()
    await user.click(joinButton())
    await dialog()
    await leave(user)
    await gone()
    expect(lobbies.joinGame).not.toHaveBeenCalled()
    await waitFor(() => expect(joinButton()).toHaveFocus())
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('asks again from the same row afterwards', async () => {
    const { lobbies, user } = await open()
    await user.click(joinButton())
    await dialog()
    await user.keyboard('{Escape}')
    await gone()
    await user.click(joinButton())
    await dialog()
    await user.type(nameField(), 'Cy')
    await user.click(sendButton())
    await waitFor(() => expect(stub()).toHaveTextContent('g1'))
    expect(lobbies.joinGame).toHaveBeenCalledTimes(1)
  })
})

describe('a signed-in visitor', () => {
  it('joins in one click with no name, the account binds the seat, and the game opens with the seat link', async () => {
    const { lobbies, user } = await open(fakeLobbies([BO]), accountStore('signed-in'))
    await user.click(joinButton())
    await waitFor(() => expect(stub()).toHaveTextContent('g1 /game/g1#seat=1&token=tok-1'))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(lobbies.joinGame).toHaveBeenCalledExactlyOnceWith('g1', {})
  })

  it('opens the bare game page when the account already held a seat and the answer has no token', async () => {
    const lobbies = fakeLobbies([BO])
    lobbies.joinGame.mockResolvedValueOnce({ game: 'g1', seat: 0, token: null })
    const { user } = await open(lobbies, accountStore('signed-in'))
    await user.click(joinButton())
    await waitFor(() => expect(stub()).toHaveTextContent(/^g1 \/game\/g1$/))
  })

  it('waits for the first answer about the session before it decides, and is not asked for a name', async () => {
    let answer: (user: UserView) => void = () => undefined
    const me = () =>
      new Promise<UserView>((resolve) => {
        answer = resolve
      })
    const store = accountStore('loading', me)
    const { lobbies, user } = await open(fakeLobbies([BO]), store)
    await user.click(joinButton())
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(lobbies.joinGame).not.toHaveBeenCalled()
    answer(ADA)
    await waitFor(() => expect(stub()).toHaveTextContent('g1'))
    expect(lobbies.joinGame).toHaveBeenCalledExactlyOnceWith('g1', {})
  })

  it('is asked for a name after all when that first answer says nobody is signed in', async () => {
    const { lobbies, user } = await open(
      fakeLobbies([BO]),
      accountStore('loading', () => Promise.reject(new Error('signed out'))),
    )
    await user.click(joinButton())
    await dialog()
    expect(lobbies.joinGame).not.toHaveBeenCalled()
  })
})

describe('a seat that is gone', () => {
  const gone409: [LobbyErrorCode, number, string][] = [
    ['full', 409, 'Someone took that seat first: the game is full.'],
    ['started', 409, 'That game has already started.'],
    ['not_found', 404, NOT_OPEN],
  ]

  it.each(gone409)(
    '(%s) is said on the page for a guest, the dialog closes and the list is read again',
    async (code, status, words) => {
      const lobbies = fakeLobbies([BO])
      lobbies.joinGame.mockRejectedValueOnce(new LobbyError(code, status, 'server words'))
      const { user } = await open(lobbies)
      expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
      lobbies.set([lobbyRow({ game: 'g1', host: 'Bo', filled: 4, total: 4 })])
      await joinAs(user, 'Cy')
      await gone()
      const alert = screen.getByRole('alert')
      expect(alert).toHaveTextContent("Could not join Bo's game")
      expect(alert).toHaveTextContent(words)
      await waitFor(() => expect(lobbies.listLobbies).toHaveBeenCalledTimes(2))
      await waitFor(() => expect(screen.getByText('Full')).toBeInTheDocument())
      expect(screen.getByRole('button', { name: "Join Bo's game, 4 of 4 seats" })).toHaveAttribute(
        'aria-disabled',
        'true',
      )
      await waitFor(() => expect(rowButton()).toHaveFocus())
      expect(screen.queryByTestId('game-stub')).not.toBeInTheDocument()
    },
  )

  it.each(gone409)(
    '(%s) is said on the page for a signed-in visitor too, who stays on their button',
    async (code, status, words) => {
      const lobbies = fakeLobbies([BO])
      lobbies.joinGame.mockRejectedValueOnce(new LobbyError(code, status, 'server words'))
      const { user } = await open(lobbies, accountStore('signed-in'))
      await user.click(joinButton())
      const alert = await screen.findByRole('alert')
      expect(alert).toHaveTextContent(words)
      await waitFor(() => expect(lobbies.listLobbies).toHaveBeenCalledTimes(2))
      expect(rowButton()).toHaveFocus()
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    },
  )

  it('says "Someone took that seat" for a row that filled', async () => {
    const lobbies = fakeLobbies([BO])
    lobbies.joinGame.mockRejectedValueOnce(new LobbyError('full', 409, 'x'))
    const { user } = await open(lobbies, accountStore('signed-in'))
    await user.click(joinButton())
    expect(await screen.findByRole('alert')).toHaveTextContent('Someone took that seat')
  })

  it('never says why a game is not open: an unknown, a private and a closed game read the same', async () => {
    expect(NOT_OPEN).not.toMatch(/code|private|invite|unknown|wrong|exist/i)
  })

  it('goes away with Dismiss, which hands focus to Refresh, and with the next Join', async () => {
    const lobbies = fakeLobbies([BO])
    lobbies.joinGame.mockRejectedValueOnce(new LobbyError('full', 409, 'x'))
    const { user } = await open(lobbies, accountStore('signed-in'))
    await user.click(joinButton())
    const alert = await screen.findByRole('alert')
    await user.click(within(alert).getByRole('button', { name: 'Dismiss' }))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Refresh' })).toHaveFocus()

    lobbies.joinGame.mockRejectedValueOnce(new LobbyError('started', 409, 'x'))
    await user.click(joinButton())
    expect(await screen.findByRole('alert')).toHaveTextContent('already started')
    lobbies.joinGame.mockRejectedValueOnce(new LobbyError('network', 0, 'x'))
    await user.click(joinButton())
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('Could not reach the server'))
    expect(screen.getAllByRole('alert')).toHaveLength(1)
  })
})

describe('a failure that sending again can mend', () => {
  const mendable: [string, LobbyError, string][] = [
    ['offline', new LobbyError('network', 0, 'x'), 'Could not reach the server.'],
    ['a busy server', new LobbyError('unavailable', 503, 'x'), 'The server is busy or restarting.'],
    ['a server error', new LobbyError('internal', 500, 'x'), 'The server ran into a problem.'],
    ['an unreadable answer', new LobbyError('bad_response', 502, 'x'), 'could not read'],
    ['a name the server refuses', new LobbyError('invalid_name', 400, 'x'), 'Choose a name of 1 to 32'],
    ['a rate limit', new LobbyError('too_many_attempts', 429, 'x', 12), 'Try again in 12 seconds.'],
  ]

  it.each(mendable)(
    '(%s) stays in the guest dialog with the name kept, and works on the second try',
    async (_what, error, words) => {
      const lobbies = fakeLobbies([BO])
      lobbies.joinGame.mockRejectedValueOnce(error)
      const { user } = await open(lobbies)
      await joinAs(user, 'Cy')
      const dialogAlert = await within(screen.getByRole('dialog')).findByRole('alert')
      expect(dialogAlert).toHaveTextContent(words)
      expect(nameField()).toHaveValue('Cy')
      expect(sendButton()).not.toHaveAttribute('aria-busy')
      expect(screen.getAllByRole('alert')).toHaveLength(1)
      expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
      await user.click(sendButton())
      await waitFor(() => expect(stub()).toHaveTextContent('g1 /game/g1#seat=1&token=tok-1'))
      expect(lobbies.joinGame).toHaveBeenCalledTimes(2)
    },
  )

  it.each(mendable)(
    '(%s) shows on the page for a signed-in visitor, whose Join is free to press again',
    async (_what, error, words) => {
      const lobbies = fakeLobbies([BO])
      lobbies.joinGame.mockRejectedValueOnce(error)
      const { user } = await open(lobbies, accountStore('signed-in'))
      await user.click(joinButton())
      expect(await screen.findByRole('alert')).toHaveTextContent(words)
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
      expect(rowButton()).not.toHaveAttribute('aria-disabled', 'true')
      await user.click(joinButton())
      await waitFor(() => expect(stub()).toHaveTextContent('g1 /game/g1#seat=1&token=tok-1'))
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    },
  )

  it('replaces the failure in the dialog with the next one rather than piling them up', async () => {
    const lobbies = fakeLobbies([BO])
    lobbies.joinGame.mockRejectedValueOnce(new LobbyError('network', 0, 'x'))
    lobbies.joinGame.mockRejectedValueOnce(new LobbyError('unavailable', 503, 'x'))
    const { user } = await open(lobbies)
    await joinAs(user, 'Cy')
    await screen.findByText(/Could not reach the server/)
    await user.click(sendButton())
    await screen.findByText(/busy or restarting/)
    expect(screen.queryByText(/Could not reach the server/)).not.toBeInTheDocument()
    expect(screen.getAllByRole('alert')).toHaveLength(1)
  })
})

describe('a row that went while the dialog was up', () => {
  it("sends focus to Refresh when Cancel finds the row's own button gone", async () => {
    const lobbies = fakeLobbies([BO, lobbyRow({ game: 'g2', host: 'Cy' })])
    const { user } = await open(lobbies)
    await user.click(joinButton())
    await dialog()
    const opener = rowButton()
    // The row's button leaves the page while the dialog is up, as a poll that drops the game would make it.
    opener?.remove()
    await user.keyboard('{Escape}')
    await gone()
    await waitFor(() => expect(screen.getByRole('button', { name: 'Refresh' })).toHaveFocus())
  })
})
