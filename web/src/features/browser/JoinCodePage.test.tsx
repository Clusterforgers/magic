// Goal: the page an invite link opens shows the code in its normal form, whatever case and hyphens the address had, above one Join button; a signed-in visitor joins in one click and a guest gives a name on the page, remembered for the next time, both through the Play page's hook, one request at a time, to the seat link in the fragment (the bare page for an account's seat); a code that cannot be one is said so without a request; a code no open game has, a full game and a started one end in a sentence with the ways to Play and to Create a game, the words for an unknown, a closed and an invite game being the same; every failure that sending again can mend stays on the form with the wait from Retry-After; and focus lands on the first control the person needs.
import { fireEvent, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import type { AuthStatus, AuthStore } from '@/features/auth/store'
import type { AccountsApi } from '@/net/accounts'
import { LobbyError } from '@/net/lobbyErrors'
import type { Joined, LobbyErrorCode, UserView } from '@/protocol/generated'
import { fakeLobbies } from '@/test-utils/lobbies'
import type { FakeLobbies } from '@/test-utils/lobbies'
import { renderInRouter } from '@/test-utils/router'
import { CODE_SHAPE, NO_GAME_WITH_CODE } from './joinErrors'
import { JoinCodePage } from './JoinCodePage'

beforeEach(() => localStorage.clear())
afterEach(() => {
  vi.restoreAllMocks()
  localStorage.clear()
})

const ADA: UserView = { id: 'u1', username: 'Ada_L' }
/** As an address can carry it, and as the server reads it. */
const TYPED = 'k7m2-9qxa'
const CODE = 'K7M29QXA'
/** What the stub game page shows after `fakeLobbies` seats a join by code. */
const CODED = 'coded-game /game/coded-game#seat=1&token=tok-1'
const HINT = 'Choose the name the other players see at the table.'

function accountStore(status: AuthStatus, me?: AccountsApi['me']): AuthStore {
  return createAuthStore({
    api: { ...offlineAccounts, ...(me === undefined ? {} : { me }) },
    initial: { status, user: status === 'signed-in' ? ADA : null },
  })
}

interface Visit {
  code?: string
  lobbies?: FakeLobbies
  store?: AuthStore
  strict?: boolean
}

async function open({ code = TYPED, lobbies = fakeLobbies(), store, strict = false }: Visit = {}) {
  const page = <JoinCodePage code={code} api={lobbies.api} />
  const view = await renderInRouter(
    store === undefined ? page : <AuthProvider store={store}>{page}</AuthProvider>,
    { strict },
  )
  return { lobbies, view, user: userEvent.setup() }
}

type User = ReturnType<typeof userEvent.setup>

const joinButton = () => screen.getByRole('button', { name: 'Join game' })
const nameField = () => screen.getByLabelText('Your name')
const playLink = () => screen.getByRole('link', { name: 'Play' })
const createLink = () => screen.getByRole('link', { name: 'Create a game' })
const stub = () => screen.getByTestId('game-stub')
const codeShown = () => document.querySelector('[data-slot="join-code"]')

/** A guest types a name and presses Join. */
async function joinAs(user: User, name: string) {
  await user.clear(nameField())
  if (name !== '') await user.type(nameField(), name)
  await user.click(joinButton())
}

describe('the page', () => {
  it('is titled Join a game and shows the code in its normal form above one Join button', async () => {
    await open()
    expect(screen.getByRole('heading', { level: 1, name: 'Join a game' })).toBeInTheDocument()
    expect(document.title).toBe('Join a game · Magic')
    expect(screen.getByText('Join code')).toBeVisible()
    expect(codeShown()).toHaveTextContent(/^K7M2-9QXA$/)
    expect(screen.getAllByRole('button')).toEqual([joinButton()])
  })

  it.each([
    ['lower case with a hyphen', 'k7m2-9qxa'],
    ['capitals without the hyphen', 'K7M29QXA'],
    ['a space in the middle', 'k7m2 9qxa'],
    ['hyphens and spaces anywhere', ' K-7M2 9q-xa '],
  ])(
    'reads %s as the one code, shows it grouped in capitals and sends it as the server reads it',
    async (_how, typed) => {
      const { lobbies, user } = await open({ code: typed, store: accountStore('signed-in') })
      expect(codeShown()).toHaveTextContent(/^K7M2-9QXA$/)
      await user.click(joinButton())
      await waitFor(() => expect(stub()).toHaveTextContent(CODED))
      expect(lobbies.joinByCode).toHaveBeenCalledExactlyOnceWith(CODE, {})
      expect(lobbies.joinGame).not.toHaveBeenCalled()
    },
  )
})

describe('a signed-in visitor', () => {
  it('has no name to give, is told whose seat it is, and lands on Join', async () => {
    const { lobbies } = await open({ store: accountStore('signed-in') })
    expect(screen.queryByLabelText('Your name')).not.toBeInTheDocument()
    expect(screen.getByText('You join as Ada_L.')).toBeVisible()
    await waitFor(() => expect(joinButton()).toHaveFocus())
    expect(lobbies.joinByCode).not.toHaveBeenCalled()
  })

  it('joins in one click with no body but the code and opens the game with the seat link', async () => {
    const { lobbies, user } = await open({ store: accountStore('signed-in') })
    await user.click(joinButton())
    await waitFor(() => expect(stub()).toHaveTextContent(CODED))
    expect(lobbies.joinByCode).toHaveBeenCalledExactlyOnceWith(CODE, {})
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('opens the bare game page when the account already held a seat and the answer has no token', async () => {
    const lobbies = fakeLobbies()
    lobbies.joinByCode.mockResolvedValueOnce({ game: 'g9', seat: 0, token: null })
    const { user } = await open({ lobbies, store: accountStore('signed-in') })
    await user.click(joinButton())
    await waitFor(() => expect(stub()).toHaveTextContent(/^g9 \/game\/g9$/))
  })

  it('waits for the first answer about the session before it shows a form, then is not asked for a name', async () => {
    let answer: (user: UserView) => void = () => undefined
    const me = () =>
      new Promise<UserView>((resolve) => {
        answer = resolve
      })
    const { lobbies } = await open({ store: accountStore('loading', me) })
    expect(screen.getByRole('status')).toHaveTextContent('Checking your sign-in…')
    expect(codeShown()).toHaveTextContent('K7M2-9QXA')
    expect(screen.queryByRole('button', { name: 'Join game' })).not.toBeInTheDocument()
    expect(screen.queryByLabelText('Your name')).not.toBeInTheDocument()
    answer(ADA)
    await waitFor(() => expect(joinButton()).toHaveFocus())
    expect(screen.queryByLabelText('Your name')).not.toBeInTheDocument()
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
    expect(lobbies.joinByCode).not.toHaveBeenCalled()
  })

  it('is asked for a name after all when that first answer says nobody is signed in', async () => {
    await open({ store: accountStore('loading', () => Promise.reject(new Error('signed out'))) })
    await waitFor(() => expect(nameField()).toHaveFocus())
    expect(screen.queryByText(/You join as/)).not.toBeInTheDocument()
  })
})

describe('a guest', () => {
  it('gives a name on the page, not in a dialog, with focus in the field and nothing sent yet', async () => {
    const { lobbies } = await open()
    await waitFor(() => expect(nameField()).toHaveFocus())
    expect(nameField()).toHaveValue('')
    expect(screen.getByText(HINT)).toBeVisible()
    expect(nameField()).toHaveAccessibleDescription(HINT)
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(lobbies.joinByCode).not.toHaveBeenCalled()
  })

  it('joins under the trimmed name and opens the game with the seat link in the fragment', async () => {
    const { lobbies, user } = await open()
    await joinAs(user, '  Cy  ')
    await waitFor(() => expect(stub()).toHaveTextContent(CODED))
    expect(lobbies.joinByCode).toHaveBeenCalledExactlyOnceWith(CODE, { name: 'Cy' })
    expect(lobbies.joinGame).not.toHaveBeenCalled()
  })

  it('joins with Enter in the name field', async () => {
    const { lobbies, user } = await open()
    await user.type(nameField(), 'Cy{Enter}')
    await waitFor(() => expect(stub()).toHaveTextContent(CODED))
    expect(lobbies.joinByCode).toHaveBeenCalledExactlyOnceWith(CODE, { name: 'Cy' })
  })

  it('opens the seat the server gave, whichever it is', async () => {
    const lobbies = fakeLobbies()
    lobbies.joinByCode.mockResolvedValueOnce({ game: 'g3', seat: 3, token: 'abc123' })
    const { user } = await open({ lobbies })
    await joinAs(user, 'Cy')
    await waitFor(() => expect(stub()).toHaveTextContent('g3 /game/g3#seat=3&token=abc123'))
  })

  it('has its name remembered for the next visit, and starts with the one it remembers', async () => {
    const first = await open()
    await joinAs(first.user, 'Cy')
    await waitFor(() => expect(stub()).toHaveTextContent(CODED))
    expect(localStorage.getItem('magic.guestName')).toBe('Cy')
    first.view.unmount()

    await open()
    expect(nameField()).toHaveValue('Cy')
    await waitFor(() => expect(nameField()).toHaveFocus())
  })

  it('joins all the same when the browser keeps nothing', async () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('full', 'QuotaExceededError')
    })
    const { lobbies, user } = await open()
    expect(nameField()).toHaveValue('')
    await joinAs(user, 'Cy')
    await waitFor(() => expect(stub()).toHaveTextContent(CODED))
    expect(lobbies.joinByCode).toHaveBeenCalledExactlyOnceWith(CODE, { name: 'Cy' })
  })

  it.each<AuthStatus>(['signed-out', 'unavailable'])(
    'is a guest when the session says %s',
    async (status) => {
      await open({ store: accountStore(status) })
      await waitFor(() => expect(nameField()).toHaveFocus())
      expect(screen.queryByText(/You join as/)).not.toBeInTheDocument()
    },
  )

  it.each([
    ['nothing', '', 'Enter a name for this seat.'],
    ['only spaces', '   ', 'Enter a name for this seat.'],
    ['33 characters', 'x'.repeat(33), 'Use at most 32 characters (this one has 33).'],
    ['a control character', 'Cy\u0007', 'Remove control or invisible characters from the name.'],
  ])('is refused %s with a reason on the field, and sends nothing', async (_what, name, reason) => {
    const { lobbies, user } = await open()
    fireEvent.change(nameField(), { target: { value: name } })
    await user.click(joinButton())
    expect(screen.getByText(reason)).toBeVisible()
    expect(nameField()).toBeInvalid()
    expect(nameField()).toHaveAccessibleDescription(reason)
    expect(nameField()).toHaveFocus()
    expect(lobbies.joinByCode).not.toHaveBeenCalled()
    await user.type(nameField(), 'y')
    expect(screen.queryByText(reason)).not.toBeInTheDocument()
    expect(screen.getByText(HINT)).toBeVisible()
  })
})

describe('one request at a time', () => {
  it('disables Join and locks the name while the request is out, and another press or Enter sends nothing', async () => {
    const lobbies = fakeLobbies()
    let answer: (joined: Joined) => void = () => undefined
    lobbies.joinByCode.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          answer = resolve
        }),
    )
    const { user } = await open({ lobbies })
    await joinAs(user, 'Cy')
    await waitFor(() => expect(joinButton()).toHaveAttribute('aria-busy', 'true'))
    expect(joinButton()).toHaveAttribute('aria-disabled', 'true')
    expect(nameField()).toHaveAttribute('readonly')
    await user.click(joinButton())
    await user.click(nameField())
    await user.keyboard('{Enter}')
    expect(lobbies.joinByCode).toHaveBeenCalledTimes(1)
    answer({ game: 'g7', seat: 2, token: 'abc' })
    await waitFor(() => expect(stub()).toHaveTextContent('g7 /game/g7#seat=2&token=abc'))
    expect(lobbies.joinByCode).toHaveBeenCalledTimes(1)
  })

  it('sends one request when two submissions arrive in the same tick', async () => {
    const lobbies = fakeLobbies()
    lobbies.joinByCode.mockImplementationOnce(() => new Promise(() => undefined))
    const { user } = await open({ lobbies, store: accountStore('signed-in') })
    await user.click(joinButton())
    const form = screen.getByRole('form', { name: 'Join a game' })
    fireEvent.submit(form)
    fireEvent.submit(form)
    expect(lobbies.joinByCode).toHaveBeenCalledTimes(1)
  })

  it('sends one request when React mounts everything twice', async () => {
    const { lobbies, user } = await open({ strict: true, store: accountStore('signed-in') })
    await user.click(joinButton())
    await waitFor(() => expect(stub()).toHaveTextContent(CODED))
    expect(lobbies.joinByCode).toHaveBeenCalledTimes(1)
  })
})

describe('a code that cannot be one', () => {
  const malformed: [string, string][] = [
    ['one too short', 'K7M2-9QX'],
    ['one too long', 'K7M2-9QXAA'],
    ['a letter the alphabet leaves out', 'K7M2-9QXI'],
    ['a digit the alphabet leaves out', 'K7M2-9QX0'],
    ['other symbols', 'K7M2_9QX'],
    ['a word', 'invite'],
    ['letters of another script', 'ПРИВЕТ12'],
    ['nothing at all', ''],
  ]

  it.each(malformed)(
    '(%s) is said so without asking the server, and leaves the ways on',
    async (_what, typed) => {
      const { lobbies } = await open({ code: typed, store: accountStore('signed-in') })
      const alert = screen.getByRole('alert')
      expect(alert).toHaveTextContent('That is not a join code')
      expect(alert).toHaveTextContent(CODE_SHAPE)
      expect(screen.getByRole('heading', { level: 1, name: 'Join a game' })).toBeInTheDocument()
      expect(screen.queryByRole('button', { name: 'Join game' })).not.toBeInTheDocument()
      expect(screen.queryByLabelText('Your name')).not.toBeInTheDocument()
      expect(codeShown()).toBeNull()
      expect(playLink()).toHaveAttribute('href', '/')
      expect(createLink()).toHaveAttribute('href', '/new')
      await waitFor(() => expect(playLink()).toHaveFocus())
      expect(playLink()).toHaveAccessibleDescription(expect.stringContaining(CODE_SHAPE))
      expect(lobbies.joinByCode).not.toHaveBeenCalled()
      expect(lobbies.joinGame).not.toHaveBeenCalled()
    },
  )
})

describe('a code the server refuses', () => {
  const noGame: [string, string][] = [
    ['a wrong code', 'wrong code'],
    ['a game that is gone', 'unknown game'],
    ['an invite game without that code', 'invite game'],
  ]
  const closed: [string, LobbyErrorCode, number, string][] = [
    ['a game that is full', 'full', 409, 'Someone took that seat first: the game is full.'],
    ['a game that started', 'started', 409, 'That game has already started.'],
  ]

  /** What the page shows once the sentence is in. */
  async function endsHere(words: string) {
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent(words)
    expect(screen.getAllByRole('alert')).toHaveLength(1)
    expect(playLink()).toHaveAttribute('href', '/')
    expect(createLink()).toHaveAttribute('href', '/new')
    expect(screen.queryByRole('button', { name: 'Join game' })).not.toBeInTheDocument()
    expect(screen.queryByLabelText('Your name')).not.toBeInTheDocument()
    // The code stays, so the person can compare it with what the host said.
    expect(codeShown()).toHaveTextContent('K7M2-9QXA')
    await waitFor(() => expect(playLink()).toHaveFocus())
    // The sentence is Play's description, so a screen reader says both when focus lands.
    expect(playLink()).toHaveAccessibleDescription(words)
    expect(screen.queryByTestId('game-stub')).not.toBeInTheDocument()
  }

  it.each(noGame)(
    '(%s) ends in the one sentence of the Play page, for a signed-in visitor',
    async (_what, said) => {
      const lobbies = fakeLobbies()
      lobbies.joinByCode.mockRejectedValueOnce(new LobbyError('not_found', 404, said))
      const { user } = await open({ lobbies, store: accountStore('signed-in') })
      await user.click(joinButton())
      await endsHere(NO_GAME_WITH_CODE)
      expect(NO_GAME_WITH_CODE).toBe('No open game with that code.')
      expect(lobbies.joinByCode).toHaveBeenCalledTimes(1)
    },
  )

  it.each(noGame)(
    '(%s) ends in the same sentence for a guest, whose name field goes',
    async (_what, said) => {
      const lobbies = fakeLobbies()
      lobbies.joinByCode.mockRejectedValueOnce(new LobbyError('not_found', 404, said))
      const { user } = await open({ lobbies })
      await joinAs(user, 'Cy')
      await endsHere(NO_GAME_WITH_CODE)
      expect(lobbies.joinByCode).toHaveBeenCalledExactlyOnceWith(CODE, { name: 'Cy' })
    },
  )

  it('never says which of an unknown game, a closed game and an invite game without that code it was', async () => {
    const pages: string[] = []
    for (const [, said] of noGame) {
      const lobbies = fakeLobbies()
      lobbies.joinByCode.mockRejectedValueOnce(new LobbyError('not_found', 404, said))
      const { user, view } = await open({ lobbies, store: accountStore('signed-in') })
      await user.click(joinButton())
      await screen.findByRole('alert')
      pages.push(document.body.textContent ?? '')
      view.unmount()
    }
    expect(new Set(pages).size).toBe(1)
    expect(pages[0]).not.toMatch(/invite|private|unknown|wrong|exist|gone|closed/i)
  })

  it.each(closed)(
    '(%s) is said in the shared words, with the same ways on',
    async (_what, code, status, words) => {
      const lobbies = fakeLobbies()
      lobbies.joinByCode.mockRejectedValueOnce(new LobbyError(code, status, 'server words'))
      const { user } = await open({ lobbies })
      await joinAs(user, 'Cy')
      await endsHere(words)
    },
  )

  const mendable: [string, LobbyError, string][] = [
    ['offline', new LobbyError('network', 0, 'x'), 'Could not reach the server.'],
    ['a busy server', new LobbyError('unavailable', 503, 'x'), 'The server is busy or restarting.'],
    ['a server error', new LobbyError('internal', 500, 'x'), 'The server ran into a problem.'],
    ['an unreadable answer', new LobbyError('bad_response', 502, 'x'), 'could not read'],
    ['a name the server refuses', new LobbyError('invalid_name', 400, 'x'), 'Choose a name of 1 to 32'],
    ['too many wrong codes', new LobbyError('too_many_attempts', 429, 'x', 12), 'Try again in 12 seconds.'],
    ['one second to wait', new LobbyError('too_many_attempts', 429, 'x', 1), 'Try again in 1 second.'],
    ['no wait named', new LobbyError('too_many_attempts', 429, 'x'), 'Try again in a minute.'],
  ]

  it.each(mendable)(
    '(%s) stays on the form for a guest with the name kept, and works on the second try',
    async (_what, error, words) => {
      const lobbies = fakeLobbies()
      lobbies.joinByCode.mockRejectedValueOnce(error)
      const { user } = await open({ lobbies })
      await joinAs(user, 'Cy')
      const alert = await screen.findByRole('alert')
      expect(alert).toHaveTextContent('Could not join')
      expect(alert).toHaveTextContent(words)
      expect(screen.getAllByRole('alert')).toHaveLength(1)
      expect(nameField()).toHaveValue('Cy')
      expect(joinButton()).not.toHaveAttribute('aria-busy')
      expect(joinButton()).not.toHaveAttribute('aria-disabled')
      expect(screen.queryByRole('link', { name: 'Play' })).not.toBeInTheDocument()
      await user.click(joinButton())
      await waitFor(() => expect(stub()).toHaveTextContent(CODED))
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
      expect(lobbies.joinByCode).toHaveBeenCalledTimes(2)
      expect(lobbies.joinByCode).toHaveBeenLastCalledWith(CODE, { name: 'Cy' })
    },
  )

  it.each(mendable)(
    '(%s) stays on the form for a signed-in visitor, whose Join keeps focus',
    async (_what, error, words) => {
      const lobbies = fakeLobbies()
      lobbies.joinByCode.mockRejectedValueOnce(error)
      const { user } = await open({ lobbies, store: accountStore('signed-in') })
      await user.click(joinButton())
      expect(await screen.findByRole('alert')).toHaveTextContent(words)
      expect(joinButton()).toHaveFocus()
      expect(joinButton()).not.toHaveAttribute('aria-busy')
      expect(screen.queryByRole('link', { name: 'Play' })).not.toBeInTheDocument()
      await user.click(joinButton())
      await waitFor(() => expect(stub()).toHaveTextContent(CODED))
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
      expect(lobbies.joinByCode).toHaveBeenCalledTimes(2)
    },
  )

  it('takes the failure off the page while the next try is out', async () => {
    const lobbies = fakeLobbies()
    lobbies.joinByCode.mockRejectedValueOnce(new LobbyError('network', 0, 'x'))
    lobbies.joinByCode.mockImplementationOnce(() => new Promise(() => undefined))
    const { user } = await open({ lobbies, store: accountStore('signed-in') })
    await user.click(joinButton())
    await screen.findByRole('alert')
    await user.click(joinButton())
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(joinButton()).toHaveAttribute('aria-busy', 'true')
  })

  it('replaces one failure with the next rather than piling them up', async () => {
    const lobbies = fakeLobbies()
    lobbies.joinByCode.mockRejectedValueOnce(new LobbyError('network', 0, 'x'))
    lobbies.joinByCode.mockRejectedValueOnce(new LobbyError('unavailable', 503, 'x'))
    const { user } = await open({ lobbies, store: accountStore('signed-in') })
    await user.click(joinButton())
    await screen.findByText(/Could not reach the server/)
    await user.click(joinButton())
    await screen.findByText(/busy or restarting/)
    expect(screen.queryByText(/Could not reach the server/)).not.toBeInTheDocument()
    expect(screen.getAllByRole('alert')).toHaveLength(1)
  })
})
