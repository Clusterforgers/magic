import { createContext, useContext } from 'react'
import type { Answer, Decision, DecisionId, ObjectId } from '@/protocol/generated'
import type { DecisionName } from '../turnText'

/** The cards a prompt has selected, in the order they were chosen. Empty again whenever the decision changes. */
export interface Selection {
  ids: ReadonlySet<ObjectId>
  has(id: ObjectId): boolean
  /** Adds the card, or removes it when it is selected already. */
  toggle(id: ObjectId): void
  /** Replaces the selection. */
  set(ids: Iterable<ObjectId>): void
  clear(): void
}

/** What every prompt gets from the host. */
export interface PromptApi {
  /** The id the answer echoes; a new id gives the prompt a fresh start. */
  id: DecisionId
  /** The kind's name, such as `Priority`. */
  name: DecisionName
  decision: Decision
  /**
   * Sends the answer, once: false when the connection refused it, when this decision was already answered, or when
   * the server has moved on to another decision.
   */
  answer(answer: Answer): boolean
  selection: Selection
}

/** Provided by `PromptHost` around the prompt of the decision this seat faces. */
export const PromptContext = createContext<PromptApi | null>(null)

/** The decision being answered, its once-only `answer` and the shared card selection; throws outside a prompt. */
export function usePrompt(): PromptApi {
  const value = useContext(PromptContext)
  if (value === null) throw new Error('usePrompt must be used inside a prompt')
  return value
}
