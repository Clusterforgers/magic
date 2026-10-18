import { useEffect, useRef, useState } from 'react'
import { useGame, usePlay } from '@/features/game/gameContext'
import { previewKey } from '@/play/preview'
import type { Answer, DecisionId } from '@/protocol/generated'

/** Quiet time after the choice last changed before the engine is asked, so a run of clicks asks once. */
export const DEBOUNCE_MS = 80
/** How long a prompt waits for the engine's answer before it stops saying the outcome is being checked. */
export const GIVE_UP_MS = 1500

/**
 * Where the preview of the choice stands: `idle` while nothing is chosen, `checking` while the engine has not
 * answered (or the question is still waiting out the debounce), `ready` once its forecast is in the store, and
 * `unavailable` when it will not come: the engine had none to give, the session could not ask, or it took too long.
 */
export type PreviewStatus = 'idle' | 'checking' | 'ready' | 'unavailable'

/**
 * Asks the engine for the preview of `answer`, the answer a prompt would send for what it has chosen so far, each time
 * that choice changes (null: nothing is chosen, nothing is asked). The question waits `DEBOUNCE_MS` for a quiet moment,
 * so the latest choice wins; the reply is kept in the store with the key of the answer it is for, which is how a reply
 * for an earlier choice is told from the one that counts. The forecast itself is read through `useBlockPreview`.
 */
export function usePreviewRequest(decision: DecisionId, answer: Answer | null): PreviewStatus {
  const { session } = useGame()
  const key = answer === null ? null : previewKey(answer)
  // The timer asks for the answer of the moment it fires, which the key says is the one it was set for.
  const latest = useRef(answer)
  useEffect(() => {
    latest.current = answer
  })
  const reply = usePlay((state) => state.preview)
  const [gaveUp, setGaveUp] = useState<string | null>(null)

  useEffect(() => {
    if (key === null) return
    let giveUp: number | undefined
    const ask = window.setTimeout(() => {
      const wanted = latest.current
      if (wanted === null || !session.preview(decision, wanted)) {
        setGaveUp(key)
        return
      }
      giveUp = window.setTimeout(() => setGaveUp(key), GIVE_UP_MS)
    }, DEBOUNCE_MS)
    return () => {
      window.clearTimeout(ask)
      window.clearTimeout(giveUp)
      setGaveUp(null)
    }
  }, [session, decision, key])

  if (key === null) return 'idle'
  if (reply !== null && reply.decision === decision && reply.key === key) {
    return reply.forecast === null ? 'unavailable' : 'ready'
  }
  return gaveUp === key ? 'unavailable' : 'checking'
}
