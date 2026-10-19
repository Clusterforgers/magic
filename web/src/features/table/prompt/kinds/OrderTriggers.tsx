import { useEffect, useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  answerOf,
  defaultOrder,
  isDefaultOrder,
  moved,
  orderSentence,
  placeWords,
  triggerLabels,
} from '../orderText'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'

type Move = 'up' | 'down'

/**
 * OrderTriggers: the triggers are listed in the order they resolve, the top one first, with the engine's own order
 * (the last to trigger resolves first) already in place. Up and Down move one trigger a place and keep focus on the
 * button that was pressed, the sentence says where the trigger went, and Confirm order sends the list as the order
 * they go on the stack. Default order puts the list back.
 */
export const OrderTriggersPrompt: PromptComponent<'OrderTriggers'> = ({ options }) => {
  const { answer } = usePrompt()
  const entries = options.triggers
  const count = entries.length
  const sources = useObjects(useMemo(() => entries.map((entry) => entry.source.id), [entries]))
  const labels = useMemo(
    () =>
      triggerLabels(
        entries,
        sources.map((source) => source?.name),
      ),
    [entries, sources],
  )

  // The trigger at each place of the list, by its index among the options.
  const [shown, setShown] = useState<readonly number[]>(() => defaultOrder(count))
  const [note, setNote] = useState<string | null>(null)
  const pressed = useRef<{ entry: number; move: Move } | null>(null)

  const move = (place: number, by: -1 | 1): void => {
    const entry = shown[place]
    if (entry === undefined) return
    pressed.current = { entry, move: by === -1 ? 'up' : 'down' }
    setShown(moved(shown, place, by))
    setNote(`${labels[entry] ?? 'The trigger'} is ${placeWords(place + by, count)}.`)
  }
  // A moved row is a new place in the page, which can drop focus; it goes back to the button just pressed. A button at
  // an end stays focusable (it only reads as unavailable), so pressing it again changes nothing instead of sending the
  // trigger back the other way.
  useEffect(() => {
    const last = pressed.current
    pressed.current = null
    if (last === null) return
    document
      .querySelector<HTMLButtonElement>(`button[data-entry="${last.entry}"][data-move="${last.move}"]`)
      ?.focus()
  }, [shown])

  return (
    <PromptLayout text={orderSentence(count, note)}>
      <Button className="h-10 px-4" onClick={() => answer({ OrderTriggers: answerOf(shown) })}>
        Confirm order
      </Button>
      <Button
        variant="outline"
        className="h-10 px-4"
        disabled={isDefaultOrder(shown)}
        onClick={() => {
          setShown(defaultOrder(count))
          setNote(null)
        }}
      >
        Default order
      </Button>
      <ol
        aria-label="Your triggers, the one that resolves first on top"
        data-slot="trigger-order"
        className="grid max-h-60 basis-full gap-1 overflow-y-auto"
      >
        {shown.map((entry, place) => {
          const label = labels[entry] ?? 'A trigger'
          return (
            <li key={entry} className="flex items-center justify-between gap-2 text-sm">
              <span className="min-w-0">
                <span className="text-muted-foreground">{place + 1}.</span> {label}
                {place === 0 ? <span className="text-muted-foreground"> (resolves first)</span> : null}
                {place === count - 1 && count > 1 ? (
                  <span className="text-muted-foreground"> (resolves last)</span>
                ) : null}
              </span>
              <span className="flex shrink-0 gap-1">
                <Button
                  size="sm"
                  variant="ghost"
                  data-entry={entry}
                  data-move="up"
                  aria-label={`Move ${label} up`}
                  disabled={place === 0}
                  focusableWhenDisabled
                  onClick={() => move(place, -1)}
                >
                  Up
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  data-entry={entry}
                  data-move="down"
                  aria-label={`Move ${label} down`}
                  disabled={place === count - 1}
                  focusableWhenDisabled
                  onClick={() => move(place, 1)}
                >
                  Down
                </Button>
              </span>
            </li>
          )
        })}
      </ol>
    </PromptLayout>
  )
}
