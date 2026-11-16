import { useEffect } from 'react'
import { useGame } from '@/features/game/gameContext'
import { indexById } from '@/play/view'
import { scriptStates } from './layoutScript'

/** What `?script=layout` puts on `window`: the number of states and a step to one of them. */
export interface TableScript {
  count: number
  names: string[]
  go: (index: number) => boolean
}

/** Exposes the layout script over the loaded table as `window.__tableScript` (`?script=layout`). */
export function DevScript({ on }: { on: boolean }) {
  const { session, lookup } = useGame()
  useEffect(() => {
    const { view, log, chat } = session.store.getState()
    if (!on || view === null) return undefined
    const names = lookup.game?.seats.map((seat) => seat.name) ?? []
    const states = scriptStates({ view, log: [...log], chat: [...chat], names })
    const script: TableScript = {
      count: states.length,
      names: states.map((state) => state.name),
      go(index) {
        const state = states[index]
        if (state === undefined) return false
        const shown = state.fixture.view
        session.store.setState({
          view: shown,
          objectsById: indexById(shown.objects),
          pending: state.pending,
          clock: null,
        })
        return true
      },
    }
    Object.assign(window, { __tableScript: script })
    return () => {
      Reflect.deleteProperty(window, '__tableScript')
    }
  }, [on, session, lookup])
  return null
}
