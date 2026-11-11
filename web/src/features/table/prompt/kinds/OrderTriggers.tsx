import { useEffect, useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { useGame } from '@/features/game/gameContext'
import { orderRule } from '../../autoSkips'
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
 * they go on the stack. Default order puts the list back. "Always use this order" asks the server to put the same set of
 * triggers on in the confirmed order for the rest of the game, unasked.
 */
export const OrderTriggersPrompt: PromptComponent<'OrderTriggers'> = ({ options }) => {
  const { answer } = usePrompt()
  const { session } = useGame()
  const [always, setAlways] = useState(false)
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

  const names = useMemo(() => sources.map((source) => source?.name), [sources])
  // A rule keys each trigger by its source's name, so it is offered only when every source has one.
  const keyed = orderRule(entries, names, defaultOrder(count)) !== null
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
      <Button
        className="h-10 px-4"
        onClick={() => {
          const order = answerOf(shown)
          const rule = always ? orderRule(entries, names, order) : null
          if (rule !== null) session.autoSkip(rule)
          answer({ OrderTriggers: order })
        }}
      >
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
      {keyed ? (
        <label className="flex h-10 items-center gap-2 text-sm">
          <input
            type="checkbox"
            data-slot="always-order"
            className="size-4 accent-primary"
            checked={always}
            onChange={(event) => setAlways(event.target.checked)}
          />
          Always use this order
        </label>
      ) : null}
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
