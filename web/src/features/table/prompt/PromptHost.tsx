import { useCallback, useMemo, useRef, useState } from 'react'
import type { ComponentType } from 'react'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { PendingDecision } from '@/play/store'
import type { Answer, DecisionKind, ObjectId } from '@/protocol/generated'
import { decisionName } from '../turnText'
import { ownDecision } from './ownDecision'
import { PromptContext } from './PromptContext'
import type { PromptApi, Selection } from './PromptContext'
import { PROMPTS } from './registry'

const NOTHING: ReadonlySet<ObjectId> = new Set()

/** Draws the prompt for `kind` with that kind's options. */
function PromptFor({ kind }: { kind: DecisionKind }) {
  const name = decisionName(kind)
  // `PROMPTS` pairs each name with a component of that kind's options; a lookup by a runtime key cannot keep the pairing.
  const Prompt = PROMPTS[name] as ComponentType<{ options: unknown }>
  return <Prompt options={(kind as Record<string, unknown>)[name]} />
}

/** One decision's worth of prompt state; `PromptHost` keys it by decision id, so a new decision starts it afresh. */
function PromptScope({ pending }: { pending: PendingDecision }) {
  const { session } = useGame()
  const { id, decision } = pending
  const [ids, setIds] = useState(NOTHING)
  const sent = useRef(false)

  const selection = useMemo<Selection>(
    () => ({
      ids,
      has: (card) => ids.has(card),
      toggle: (card) =>
        setIds((current) => {
          const next = new Set(current)
          if (!next.delete(card)) next.add(card)
          return next
        }),
      set: (cards) => setIds(new Set(cards)),
      clear: () => setIds(NOTHING),
    }),
    [ids],
  )

  const answer = useCallback(
    (given: Answer): boolean => {
      // The store may have moved to another decision before this one was redrawn; an answer built for the old options
      // must not reach the new one.
      if (sent.current || session.store.getState().pending?.id !== id) return false
      sent.current = session.answer(given)
      return sent.current
    },
    [session, id],
  )

  const name = decisionName(decision.kind)
  const api = useMemo<PromptApi>(
    () => ({ id, name, decision, answer, selection }),
    [id, name, decision, answer, selection],
  )
  return (
    <PromptContext value={api}>
      <div
        data-slot="decision-line"
        data-decision={name}
        data-decision-id={id}
        className="grid max-w-full min-w-0 gap-1.5 rounded-md bg-muted px-3 py-2 sm:max-w-xl"
      >
        <PromptFor kind={decision.kind} />
      </div>
    </PromptContext>
  )
}

/**
 * Fills the decision line of the centre strip with the prompt for the decision this seat must answer, and draws
 * nothing when there is none. The prompt is keyed by decision id: a new decision remounts it with an empty selection.
 */
export function PromptHost() {
  const pending = usePlay(ownDecision)
  if (pending === null) return null
  return <PromptScope key={pending.id} pending={pending} />
}
