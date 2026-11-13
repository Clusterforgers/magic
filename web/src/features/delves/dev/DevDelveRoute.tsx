/** `/dev/delve?state=…`: the run page on a fixture run, with no server, to look at each state of a run. */
import { getRouteApi } from '@tanstack/react-router'
import { useMemo } from 'react'
import { fakeDelves } from '../fakeDelves'
import { DelvePage } from '../DelvePage'
import { DEV_CONTENT, devState, devView } from './devDelve'
import { devDecks } from './devDecks'

const route = getRouteApi('/dev/delve')

export function DevDelveRoute() {
  const { state } = route.useSearch()
  const client = useMemo(() => {
    const fake = fakeDelves(devView(devState(state)))
    return { ...fake, content: async () => DEV_CONTENT }
  }, [state])
  return <DelvePage key={state} id="dev" client={client} loadDecks={devDecks} />
}
