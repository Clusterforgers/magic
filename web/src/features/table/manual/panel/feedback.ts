/** What a manual action tells its sender: sent, taken, refused with the server's reason, or never sent. */
import { useCallback, useEffect, useRef, useState } from 'react'
import { useGame } from '@/features/game/gameContext'
import type { ManualAction } from '@/protocol/generated'
import type { ManualResult } from '@/play/manualLedger'

/** The line a row or a menu shows under its button. */
export type Feedback =
  | { state: 'sending'; what: string }
  | { state: 'done'; what: string }
  | { state: 'refused'; reason: string }
  | { state: 'lost'; reason: string }
  | { state: 'offline' }

/** The reason as a sentence: trimmed, with a closing full stop. */
function sentence(reason: string): string {
  const text = reason.trim()
  return /[.!?]$/.test(text) ? text : `${text}.`
}

/** The words of a feedback line. */
export function feedbackWords(feedback: Feedback): string {
  switch (feedback.state) {
    case 'sending':
      return `Sending: ${feedback.what}…`
    case 'done':
      return `Done: ${feedback.what}.`
    case 'refused':
      return `Refused: ${sentence(feedback.reason)}`
    case 'lost':
      return `Not confirmed: ${sentence(feedback.reason)}`
    case 'offline':
      return 'Not sent: the connection is not open.'
  }
}

/** The feedback a session's answer stands for; `what` names the action in the button's words. */
export function feedbackOf(result: ManualResult, what: string): Feedback {
  if (result.ok) return { state: 'done', what }
  return result.lost === true
    ? { state: 'lost', reason: result.reason }
    : { state: 'refused', reason: result.reason }
}

/** The sending of manual actions from one row or menu, with the line that says what became of the newest. */
export interface ManualSender {
  feedback: Feedback | null
  /** Sends `action` (named `what` in the line); `after` runs once the server took it. */
  send(action: ManualAction, what: string, after?: () => void): void
  /** Forgets the line, such as when the row's inputs change. */
  clear(): void
}

/**
 * A sender for one row or menu: the newest action's answer is the line. A refusal whose line is no longer shown (the
 * row closed, an input changed, a newer action sent) lands in the table notice, so no refusal goes unsaid.
 */
export function useManualSender(): ManualSender {
  const { session } = useGame()
  const [feedback, setFeedback] = useState<Feedback | null>(null)
  const newest = useRef(0)
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  const send = useCallback(
    (action: ManualAction, what: string, after?: () => void): void => {
      newest.current += 1
      const mine = newest.current
      setFeedback({ state: 'sending', what })
      const sent = session.manual(action, (result) => {
        const line = feedbackOf(result, what)
        if (mounted.current && mine === newest.current) {
          setFeedback(line)
          if (result.ok) after?.()
        } else if (!result.ok) {
          session.store.setState({ notice: feedbackWords(line) })
        }
      })
      if (!sent) setFeedback({ state: 'offline' })
    },
    [session],
  )
  const clear = useCallback((): void => {
    newest.current += 1
    setFeedback(null)
  }, [])
  return { feedback, send, clear }
}
