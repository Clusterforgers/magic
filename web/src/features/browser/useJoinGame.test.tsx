// Goal: a join by code that is given the guest's name up front sends it for a guest without a question, sends none for a signed-in account whatever it was given, waits for a session that is still loading before it decides, and still asks a guest who gave no name; the failure it keeps says whether the code is spent for good.
import { act, waitFor } from '@testing-library/react'
import { useEffect } from 'react'
import { describe, expect, it } from 'vitest'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import type { AuthStatus } from '@/features/auth/store'
import { LobbyError } from '@/net/lobbyErrors'
import type { UserView } from '@/protocol/generated'
import { fakeLobbies } from '@/test-utils/lobbies'
import { renderInRouter } from '@/test-utils/router'
import { useJoinGame } from './useJoinGame'
import type { JoinGame } from './useJoinGame'

const ADA: UserView = { id: 'u1', username: 'Ada_L' }
const CODE = 'K7M29QXA'

/** Renders the hook on a store that starts as `status`; `me` answers while that is `loading`. */
async function mount(status: AuthStatus, me?: () => Promise<UserView>) {
  const lobbies = fakeLobbies()
  const store = createAuthStore({
    api: { ...offlineAccounts, ...(me === undefined ? {} : { me }) },
    initial: { status, user: status === 'signed-in' ? ADA : null },
  })
  const hook: { current: JoinGame | null } = { current: null }
  function Probe() {
    const join = useJoinGame(lobbies.api)
    // Written after the render: a render must not change what lives outside it.
    useEffect(() => {
      hook.current = join
    })
    return null
  }
  await renderInRouter(
    <AuthProvider store={store}>
      <Probe />
    </AuthProvider>,
  )
  const join = (): JoinGame => {
    if (hook.current === null) throw new Error('the probe has not rendered')
    return hook.current
  }
  return { lobbies, join }
}

describe('a join by code with a name', () => {
  const sends: [string, AuthStatus, string | undefined, object][] = [
    ['a guest who gave a name', 'signed-out', 'Cy', { name: 'Cy' }],
    ['a guest of a server without accounts', 'unavailable', 'Cy', { name: 'Cy' }],
    ['an account that gave no name', 'signed-in', undefined, {}],
    ['an account that was given a name', 'signed-in', 'Cy', {}],
  ]

  it.each(sends)(
    '(%s) sends the body the account or the name makes, and asks nothing',
    async (_who, status, name, body) => {
      const { lobbies, join } = await mount(status)
      act(() => join().startCode(CODE, null, name))
      await waitFor(() => expect(lobbies.joinByCode).toHaveBeenCalledExactlyOnceWith(CODE, body))
      expect(join().asking).toBeNull()
    },
  )

  it('asks a guest who gave no name, and sends nothing until the answer comes', async () => {
    const { lobbies, join } = await mount('signed-out')
    act(() => join().startCode(CODE, null))
    await waitFor(() => expect(join().asking).not.toBeNull())
    expect(lobbies.joinByCode).not.toHaveBeenCalled()
    expect(join().busy).toBe(false)
  })

  it('waits for a session that is still loading, and an account that turns up is sent without the name', async () => {
    let answer: (user: UserView) => void = () => undefined
    const { lobbies, join } = await mount(
      'loading',
      () =>
        new Promise<UserView>((resolve) => {
          answer = resolve
        }),
    )
    act(() => join().startCode(CODE, null, 'Cy'))
    expect(lobbies.joinByCode).not.toHaveBeenCalled()
    answer(ADA)
    await waitFor(() => expect(lobbies.joinByCode).toHaveBeenCalledExactlyOnceWith(CODE, {}))
  })

  it('sends the name when the loading session turns out to be nobody', async () => {
    const { lobbies, join } = await mount('loading', () => Promise.reject(new Error('signed out')))
    act(() => join().startCode(CODE, null, 'Cy'))
    await waitFor(() => expect(lobbies.joinByCode).toHaveBeenCalledExactlyOnceWith(CODE, { name: 'Cy' }))
  })
})

describe('the failure of a join by code', () => {
  const failures: [string, LobbyError, boolean][] = [
    ['no open game has the code', new LobbyError('not_found', 404, 'x'), true],
    ['the game is full', new LobbyError('full', 409, 'x'), true],
    ['the game started', new LobbyError('started', 409, 'x'), true],
    ['the network is down', new LobbyError('network', 0, 'x'), false],
    ['the server is busy', new LobbyError('unavailable', 503, 'x'), false],
    ['too many wrong codes were tried', new LobbyError('too_many_attempts', 429, 'x', 5), false],
  ]

  it.each(failures)('is kept with its words when %s, final or not as it is', async (_what, error, final) => {
    const { lobbies, join } = await mount('signed-in')
    lobbies.joinByCode.mockRejectedValueOnce(error)
    act(() => join().startCode(CODE, null))
    await waitFor(() => expect(join().codeFailure).not.toBeNull())
    expect(join().codeFailure?.final).toBe(final)
    expect(join().codeProblem).toBe(join().codeFailure?.text)
    expect(join().busy).toBe(false)
    act(() => join().dismissCode())
    expect(join().codeFailure).toBeNull()
    expect(join().codeProblem).toBeNull()
  })
})
