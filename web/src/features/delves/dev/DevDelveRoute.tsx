/**
 * `/dev/delve?state=…`: the run page on a fixture run, with no server, to look at each state of a run; `state=list`
 * shows the Delves page itself, signed in, with the fixture leaderboard.
 */
import { getRouteApi } from '@tanstack/react-router'
import { useMemo } from 'react'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { fakeDelves } from '../fakeDelves'
import { DelvePage } from '../DelvePage'
import { DelvesPage } from '../DelvesPage'
import { DEV_CONTENT, devState, devView } from './devDelve'
import { pickerDecks } from '../fakePickerDecks'

const route = getRouteApi('/dev/delve')

const signedIn = createAuthStore({
  api: offlineAccounts,
  initial: { status: 'signed-in', user: { id: 'dev', username: 'Ana', is_admin: false } },
})

export function DevDelveRoute() {
  const { state } = route.useSearch()
  const client = useMemo(() => {
    const fake = fakeDelves(devView(devState(state)))
    return { ...fake, content: async () => DEV_CONTENT }
  }, [state])
  if (state === 'list') {
    return (
      <AuthContext.Provider value={signedIn}>
        <DelvesPage client={client} />
      </AuthContext.Provider>
    )
  }
  return <DelvePage key={state} id="dev" client={client} loadDecks={pickerDecks} />
}
