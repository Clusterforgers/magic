import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { KeyboardEvent } from 'react'
import { Button } from '@/components/ui/button'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { usePreviewHandlers } from '../../usePreviewHandlers'
import { cardPlaceWords, ORDER_KEYS, orderSentence, orderTitle } from '../cardsText'
import { askerWords, numberKey } from '../choiceText'
import { uniqueNames } from '../combatText'
import { orderPreview, pileHeading, pileOf } from '../lookText'
import { moved } from '../orderText'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import { PromptPicture } from '../PromptPicture'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { usePromptKeys } from '../usePromptKeys'

type Move = 'up' | 'down'

/** Up to this many cards are drawn with their cost and type line; more use the compact face. */
const NORMAL_UP_TO = 5

/** The arrow keys that move the focused card toward the top (-1) or the bottom (1) of the library. */
const ARROWS: Readonly<Record<string, -1 | 1>> = { ArrowUp: -1, ArrowLeft: -1, ArrowDown: 1, ArrowRight: 1 }

interface OrderRowProps {
  id: ObjectId
  object: ObjectView | undefined
  label: string
  place: number
  count: number
  size: 'normal' | 'compact'
  onMove(place: number, by: -1 | 1): void
}

/** One card of the strip: its place, its picture and the two buttons that move it. Hover and focus preview the card. */
function OrderRow({ id, object, label, place, count, size, onMove }: OrderRowProps) {
  const preview = usePreviewHandlers(id)
  return (
    <li
      data-card={id}
      onFocus={preview.onFocus}
      onBlur={preview.onBlur}
      className="flex min-w-0 flex-col items-center gap-1"
    >
      <span className="text-xs tabular-nums">
        <span className="text-muted-foreground">{place + 1}.</span>
        <span className="sr-only"> {label}</span>
        {place === 0 ? <span className="text-muted-foreground"> (first)</span> : null}
        {place === count - 1 && count > 1 ? <span className="text-muted-foreground"> (last)</span> : null}
      </span>
      <span onPointerEnter={preview.onPointerEnter} onPointerLeave={preview.onPointerLeave}>
        <PromptPicture object={object} size={size} />
      </span>
      <span className="flex shrink-0 gap-1">
        <Button
          size="icon-xs"
          variant="outline"
          data-card={id}
          data-move="up"
          aria-label={`Move ${label} up`}
          title="Toward the top"
          disabled={place === 0}
          focusableWhenDisabled
          onClick={() => onMove(place, -1)}
        >
          <ChevronLeft aria-hidden="true" />
        </Button>
        <Button
          size="icon-xs"
          variant="outline"
          data-card={id}
          data-move="down"
          aria-label={`Move ${label} down`}
          title="Toward the bottom"
          disabled={place === count - 1}
          focusableWhenDisabled
          onClick={() => onMove(place, 1)}
        >
          <ChevronRight aria-hidden="true" />
        </Button>
      </span>
    </li>
  )
}

/**
 * OrderCards: the cards of a scry, a put-back or a bottoming, as pictures in a strip the player puts in order, the card
 * nearest the top of the library first (left). The buttons beside each card move it one place toward the top or the
 * bottom; Alt with the arrow keys does the same from the focused card and a number key goes to the card at that place.
 * A moved card keeps focus on the button pressed, the sentence says where it went and a line under the strip gives the
 * result from the top down. Original order puts the list back and Confirm order sends every card once, the top of the
 * list first. The server's answer at the deadline is the order offered.
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
    const by = ARROWS[event.key]
    if (by === undefined || !(event.target instanceof Element)) return
    const raw = event.target.closest('[data-card]')?.getAttribute('data-card')
    const place = raw === null || raw === undefined ? -1 : shown.indexOf(Number(raw))
    if (place < 0) return
    event.preventDefault()
    move(place, by)
  }
  // A number key goes to the card at that place, on its first button that works.
  const goTo = (place: number): void => {
    const id = shown[place]
    if (id === undefined) return
    const buttons = list.current?.querySelectorAll<HTMLButtonElement>(`button[data-card="${id}"]`)
    const target =
      buttons === undefined ? undefined : [...buttons].find((b) => b.getAttribute('aria-disabled') !== 'true')
    target?.focus()
  }
  usePromptKeys(
    Object.fromEntries(shown.slice(0, 10).map((_, place) => [numberKey(place) ?? '', () => goTo(place)])),
  )

  const unchanged = shown.every((id, place) => id === cards[place])
  const words = options.text ?? ''
  const pile = pileOf(words)
  const sentence = orderSentence(askerWords(source?.name), words === '' ? orderTitle(count) : words, note)
  const size = count <= NORMAL_UP_TO ? 'normal' : 'compact'
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
      <p data-slot="order-pile" className="min-w-0 flex-1 truncate text-xs font-medium text-muted-foreground">
        {pileHeading(pile)}: first on the left
      </p>
      <ol
        ref={list}
        aria-label="Your cards in order, the first on top"
        data-slot="card-order"
        data-pile={pile}
        onKeyDown={onKeyDown}
        // What the stack, the sentence, the buttons and the clock leave of the sheet (`--sheet-max`) is the strip's.
        className="flex max-h-[max(6rem,calc(var(--sheet-max,20rem)_-_13rem))] basis-full flex-wrap content-start gap-2 overflow-y-auto p-0.5"
      >
        {shown.map((id, place) => (
          <OrderRow
            key={id}
            id={id}
            object={objects.get(id)}
            label={labels.get(id) ?? 'A card'}
            place={place}
            count={count}
            size={size}
            onMove={move}
          />
        ))}
      </ol>
      {count === 0 ? null : (
        <p data-slot="order-preview" aria-hidden="true" className="basis-full text-xs text-muted-foreground">
          {orderPreview(
            shown.map((id) => labels.get(id) ?? 'A card'),
            pile,
          )}
        </p>
      )}
    </PromptLayout>
  )
}
