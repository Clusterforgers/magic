/** A session for a replay: nothing connects and every action is refused, since a finished game takes no answer. */
import type { Session } from '@/play/session'
import type { PlayStore } from '@/play/store'

/** A session over `store` that does nothing: the replay page alone writes the store. */
export function replaySession(store: PlayStore): Session {
  const no = () => false
  return {
    store,
    connect: () => undefined,
    disconnect: () => undefined,
    ready: no,
    notReady: no,
    answer: no,
    preview: no,
    manual: no,
    concede: no,
    setStop: no,
    setFullControl: no,
    setEndTurnByHand: no,
    setAutoChoices: no,
    autoYield: no,
    clearAutoYield: no,
    autoSkip: no,
    cancelAutoSkip: no,
    castWhenAsked: no,
    yieldUntil: no,
    cancelYield: no,
    chat: no,
    resync: no,
    clearDueLines: () => undefined,
    dismissNotice: () => store.setState({ notice: null }),
  }
}
