import { useLayoutEffect, useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import type { Answer, ObjectId } from '@/protocol/generated'
import { countedText, refusalText } from './countedText'
import { markCards, useCardMarks } from './marks'
import { useOwnCardIds } from './ownCards'
import { usePrompt } from './PromptContext'
import { PromptLayout } from './PromptLayout'

/** Props of `CountedChoice`. */
export interface CountedChoiceProps {
  /** The cards that may be chosen; the viewer's other cards are dimmed. */
  offered: readonly ObjectId[]
  /** Exactly this many must be chosen (never more than are offered). */
  count: number
  /** What is done with the chosen cards, as a verb phrase after "to": "discard". */
  what: string
  /** A sentence ahead of the first request, with its trailing space. */
  intro?: string
  /** The answer for the chosen cards, in the order they were chosen. */
  answerWith(chosen: ObjectId[]): Answer
}

/**
 * A prompt that picks exactly `count` of the `offered` cards (bottoming, discarding): a click or Enter/Space on a card
 * toggles it, the sentence counts "2 of 3 chosen", a card beyond the count is refused with a spoken hint, and Confirm
 * waits until the count is met.
 */
export function CountedChoice({ offered, count, what, intro, answerWith }: CountedChoiceProps) {
  const { answer, selection } = usePrompt()
  const own = useOwnCardIds()
  const need = Math.min(count, offered.length)
  const offeredSet = useMemo(() => new Set(offered), [offered])
  const chosen = useMemo(
    () => [...selection.ids].filter((id) => offeredSet.has(id)),
    [selection.ids, offeredSet],
  )
  // `times` keys the hint, so a repeated refusal replaces it and is spoken again.
  const [refusal, setRefusal] = useState({ shown: false, times: 0 })

  const marks = useMemo(() => markCards(own, need > 0 ? offered : [], chosen), [own, need, offered, chosen])
  // What has been chosen including clicks the next render has not shown yet, so two quick clicks cannot pass the count.
  const latest = useRef(chosen)
  useLayoutEffect(() => {
    latest.current = chosen
  })
  const choose = (id: ObjectId): void => {
    if (!offeredSet.has(id)) return
    const now = latest.current
    if (!now.includes(id) && now.length >= need) {
      setRefusal(({ times }) => ({ shown: true, times: times + 1 }))
      return
    }
    setRefusal((current) => (current.shown ? { ...current, shown: false } : current))
    latest.current = now.includes(id) ? now.filter((card) => card !== id) : [...now, id]
    selection.set(latest.current)
  }
  useCardMarks(marks, choose, { toggles: true })

  const ready = chosen.length === need
  const confirm = (): void => {
    if (ready) answer(answerWith(chosen))
  }
  const text = (
    <>
      {countedText(need, chosen.length, what, intro)}
      {refusal.shown ? (
        <>
          {' '}
          <span key={refusal.times} data-slot="prompt-hint" className="font-semibold">
            {refusalText(need)}
          </span>
        </>
      ) : null}
    </>
  )

  return (
    <PromptLayout text={text}>
      <Button className="h-10 px-4" disabled={!ready} onClick={confirm}>
        Confirm
      </Button>
    </PromptLayout>
  )
}
