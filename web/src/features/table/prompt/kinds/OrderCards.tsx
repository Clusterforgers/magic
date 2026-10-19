import { useEffect, useMemo, useRef, useState } from 'react'
import type { KeyboardEvent } from 'react'
import { Button } from '@/components/ui/button'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { costText, typeLine } from '../../cardModel'
import { usePreviewHandlers } from '../../usePreviewHandlers'
import { cardPlaceWords, ORDER_KEYS, orderSentence, orderTitle } from '../cardsText'
import { askerWords } from '../choiceText'
import { uniqueNames } from '../combatText'
import { moved } from '../orderText'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'

type Move = 'up' | 'down'

interface OrderRowProps {
  id: ObjectId
  object: ObjectView | undefined
  label: string
  place: number
  count: number
  onMove(place: number, by: -1 | 1): void
}

/** One card of the list: its place, its name and type, and the two buttons that move it. Hover and focus preview the card. */
function OrderRow({ id, object, label, place, count, onMove }: OrderRowProps) {
  const preview = usePreviewHandlers(id)
  const details =
    object === undefined
      ? ''
      : [object.mana_cost === null ? '' : costText(object.mana_cost), typeLine(object)].join(' ')
  return (
    <li
      data-card={id}
      onFocus={preview.onFocus}
      onBlur={preview.onBlur}
      className="flex items-center justify-between gap-2 text-sm"
    >
      <span
        onPointerEnter={preview.onPointerEnter}
        onPointerLeave={preview.onPointerLeave}
        className="min-w-0 flex-1 truncate"
      >
        <span className="text-muted-foreground">{place + 1}.</span>{' '}
        <span className="font-medium">{label}</span>
        {place === 0 ? <span className="text-muted-foreground"> (first)</span> : null}
        {place === count - 1 && count > 1 ? <span className="text-muted-foreground"> (last)</span> : null}
        {details.trim() === '' ? null : <span className="text-xs text-muted-foreground"> {details}</span>}
      </span>
      <span className="flex shrink-0 gap-1">
        <Button
          size="xs"
          variant="ghost"
          data-card={id}
          data-move="up"
          aria-label={`Move ${label} up`}
          disabled={place === 0}
          focusableWhenDisabled
          onClick={() => onMove(place, -1)}
        >
          Up
        </Button>
        <Button
          size="xs"
          variant="ghost"
          data-card={id}
          data-move="down"
          aria-label={`Move ${label} down`}
          disabled={place === count - 1}
          focusableWhenDisabled
          onClick={() => onMove(place, 1)}
        >
          Down
        </Button>
      </span>
    </li>
  )
}

/**
 * OrderCards: the cards of a scry, a put-back or a bottoming, in a list the player puts in order. Up and Down move a
 * card one place (Alt with the arrow keys does the same from the focused card), a moved card keeps focus on the button
 * pressed, the sentence says where it went, Original order puts the list back and Confirm order sends every card once,
 * the top of the list first. The server's answer at the deadline is the order offered.
 */
export const OrderCardsPrompt: PromptComponent<'OrderCards'> = ({ options }) => {
  const { answer } = usePrompt()
  const cards = options.cards
  const count = cards.length
  const known = useObjects(useMemo(() => [options.source, ...cards], [options.source, cards]))
  const source = known[0]
  const objects = useMemo(
    () => new Map(cards.map((id, place) => [id, known[place + 1]] as const)),
    [cards, known],
  )
  const labels = useMemo(
    () => uniqueNames(cards.map((id) => ({ id, name: objects.get(id)?.name ?? 'A card' }))),
    [cards, objects],
  )

  // The card at each place of the list, top first.
  const [shown, setShown] = useState<readonly ObjectId[]>(cards)
  const [note, setNote] = useState<string | null>(null)
  const pressed = useRef<{ id: ObjectId; move: Move } | null>(null)
  const list = useRef<HTMLOListElement>(null)

  const move = (place: number, by: -1 | 1): void => {
    const id = shown[place]
    const to = place + by
    if (id === undefined || to < 0 || to >= count) return
    pressed.current = { id, move: by === -1 ? 'up' : 'down' }
    setShown(moved(shown, place, by))
    setNote(`${labels.get(id) ?? 'The card'} is ${cardPlaceWords(to, count)}.`)
  }
  // A moved row is a new place in the page, which can drop focus; it goes back to the button just pressed.
  useEffect(() => {
    const last = pressed.current
    pressed.current = null
    if (last === null) return
    list.current
      ?.querySelector<HTMLButtonElement>(`button[data-card="${last.id}"][data-move="${last.move}"]`)
      ?.focus()
  }, [shown])
  const onKeyDown = (event: KeyboardEvent<HTMLElement>): void => {
    if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return
    if (!(event.target instanceof Element)) return
    const raw = event.target.closest('[data-card]')?.getAttribute('data-card')
    const place = raw === null || raw === undefined ? -1 : shown.indexOf(Number(raw))
    if (place < 0) return
    event.preventDefault()
    move(place, event.key === 'ArrowUp' ? -1 : 1)
  }

  const unchanged = shown.every((id, place) => id === cards[place])
  const words = options.text ?? ''
  const sentence = orderSentence(askerWords(source?.name), words === '' ? orderTitle(count) : words, note)
  return (
    <PromptLayout
      text={
        <>
          {sentence}
          <span className="sr-only"> {ORDER_KEYS}</span>
        </>
      }
    >
      <Button className="h-9 px-4" onClick={() => answer({ OrderCards: [...shown] })}>
        Confirm order
      </Button>
      <Button
        variant="outline"
        className="h-9 px-4"
        disabled={unchanged}
        onClick={() => {
          setShown(cards)
          setNote(null)
        }}
      >
        Original order
      </Button>
      <ol
        ref={list}
        aria-label="Your cards in order, the first on top"
        data-slot="card-order"
        onKeyDown={onKeyDown}
        // What the stack, the sentence, the buttons and the clock leave of the sheet (`--sheet-max`) is the list's.
        className="grid max-h-[max(4.5rem,calc(var(--sheet-max,20rem)_-_11rem))] basis-full gap-0.5 overflow-y-auto"
      >
        {shown.map((id, place) => (
          <OrderRow
            key={id}
            id={id}
            object={objects.get(id)}
            label={labels.get(id) ?? 'A card'}
            place={place}
            count={count}
            onMove={move}
          />
        ))}
      </ol>
    </PromptLayout>
  )
}
