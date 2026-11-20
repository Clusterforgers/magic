import { useCallback, useMemo, useRef, useState } from 'react'
import type { ComponentType } from 'react'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { PendingDecision } from '@/play/store'
import type { Answer, DecisionKind, ObjectId } from '@/protocol/generated'
import { decisionName } from '../turnText'
import type { DecisionName } from '../turnText'
import { ownDecision } from './ownDecision'
import { PromptContext } from './PromptContext'
import type { PromptApi, Selection } from './PromptContext'
import { UnsupportedPrompt } from './PromptLayout'
import { PROMPTS } from './registry'

const NOTHING: ReadonlySet<ObjectId> = new Set()

/**
 * The name of a decision kind: the key it came with, which is a kind of `PROMPTS` unless the server is newer than this
 * client (a kind this version does not know).
 */
function kindName(kind: DecisionKind): string {
  const [key] = Object.keys(kind)
  return key !== undefined && Object.hasOwn(PROMPTS, key) ? decisionName(kind) : (key ?? 'Unknown')
}

/** Draws the prompt for `kind` with that kind's options; a kind that is not known gets the generic prompt. */
function PromptFor({ kind }: { kind: DecisionKind }) {
  const name = kindName(kind)
  if (!Object.hasOwn(PROMPTS, name)) return <UnsupportedPrompt />
  // `PROMPTS` pairs each name with a component of that kind's options; a lookup by a runtime key cannot keep the pairing.
  const Prompt = PROMPTS[name as DecisionName] as ComponentType<{ options: unknown }>
  return <Prompt options={(kind as Record<string, unknown>)[name]} />
}

/** One decision's worth of prompt state; `PromptHost` keys it by decision id, so a new decision starts it afresh. */
function PromptScope({ pending }: { pending: PendingDecision }) {
  const { session } = useGame()
  const { id, decision, asking } = pending
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

  const name = kindName(decision.kind)
  const api = useMemo<PromptApi>(
    () => ({ id, name, decision, answer, selection, ...(asking === undefined ? {} : { asking }) }),
    [id, name, decision, answer, selection, asking],
  )
  // The prompt draws its parts into the decision's home (`PromptLayout`), so its state lives here whichever home it is in.
  return (
    <PromptContext value={api}>
      <PromptFor kind={decision.kind} />
    </PromptContext>
  )
}

/**
 * The prompt for the decision this seat must answer, mounted once by the table; it draws in the decision's home. The
 * prompt is keyed by decision id: a new decision remounts it with an empty selection.
 */
export function PromptHost() {
  const pending = usePlay(ownDecision)
  if (pending === null) return null
  return <PromptScope key={pending.id} pending={pending} />
}
