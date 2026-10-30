import { ArrowDown, ArrowUp } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { KeyboardEvent, ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { CardImage } from '@/features/cards/CardImage'
import { cardImageFallbackUrl, cardImageUrl, faceOf, imageSourceOf, missKeyOf } from '@/features/cards/images'
import { useGame } from '@/features/game/gameContext'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { uniqueNames } from '../../prompt/combatText'
import { useObjects } from '../../prompt/useCombatNames'
import {
  applyWords,
  arrangeAction,
  DESTINATIONS,
  isUnchanged,
  moveWithin,
  previewWords,
  sendTo,
  startArrangement,
} from './arrange'
import type { Arrangement, Destination } from './arrange'
import type { Command, PanelInput } from './commands'

/** The keys that send the focused card to a place, and what the list says about them. */
const PLACE_KEYS: Readonly<Record<string, Destination>> = { t: 'top', b: 'bottom', g: 'graveyard' }
const KEYS_HINT =
  'Alt with the up and down arrows moves a card in its list. T, B and G send it to the top, the bottom or the graveyard.'

interface BodyProps {
  id: string
  command: Command
  input: PanelInput
  onClose: () => void
  /** The row's plain form (an amount and Apply), shown when no cards wait or the seat looks at more. */
  plain: ReactNode
}

/**
 * The body of the Look at the top row: the amount form while no looked-at cards wait, and once they do the list of
 * those cards with where each goes (`ArrangeForm`). The seat can look at more cards from the list.
 */
export function LookTopBody(props: BodyProps) {
  const [sent, setSent] = useState<string | null>(null)
  return (
    <>
      {/* A new set of looked-at cards starts the body over. */}
      <Body key={props.input.looked.join(',')} {...props} onSent={setSent} />
      {sent === null || props.input.looked.length > 0 ? null : (
        <p role="status" data-slot="command-feedback" className="px-2 pb-2 text-xs text-muted-foreground">
          {sent}
        </p>
      )}
    </>
  )
}

function Body({ id, input, onClose, plain, onSent }: BodyProps & { onSent: (text: string) => void }) {
  const [more, setMore] = useState(false)
  if (input.looked.length === 0) return <>{plain}</>
  if (more) {
    return (
      <>
        <div className="px-2 pt-1">
          <Button size="sm" variant="outline" onClick={() => setMore(false)}>
            Back to the {input.looked.length} cards
          </Button>
        </div>
        {plain}
      </>
    )
  }
  return <ArrangeForm id={id} input={input} onClose={onClose} onMore={() => setMore(true)} onSent={onSent} />
}

interface ThumbProps {
  object: ObjectView | undefined
}

/** The card's small picture when the table has one for it; nothing otherwise. */
function Thumb({ object }: ThumbProps) {
  const source = object === undefined ? null : imageSourceOf(object)
  if (object === undefined || source === null) return null
  const face = faceOf(object)
  return (
    <CardImage
      src={cardImageUrl(source, face, 'small')}
      fallback={cardImageFallbackUrl(source, face, 'small')}
      missKey={missKeyOf(source, face, 'small')}
      alt=""
      className="aspect-[5/7] w-9 shrink-0"
    >
      <span className="block size-full rounded-sm border border-border bg-muted" />
    </CardImage>
  )
}

interface FormProps {
  id: string
  input: PanelInput
  onClose: () => void
  onMore: () => void
  onSent: (text: string) => void
}

/** Where each looked-at card goes, with Apply, Cancel and a preview line. */
function ArrangeForm({ id, input, onClose, onMore, onSent }: FormProps) {
  const { session } = useGame()
  const looked = input.looked
  const objects = useObjects(looked)
  const names = uniqueNames(looked.map((card, at) => ({ id: card, name: objects[at]?.name ?? 'A card' })))
  const nameOf = (card: ObjectId): string => names.get(card) ?? 'A card'
  const [arrangement, setArrangement] = useState<Arrangement>(() => startArrangement(looked))
  const [feedback, setFeedback] = useState<string | null>(null)
  const list = useRef<HTMLDivElement>(null)
  // A card that changes list or place is a new element, which drops focus; the control just used gets it back.
  const refocus = useRef<{ card: ObjectId; control: string } | null>(null)
  useEffect(() => {
    const target = refocus.current
    refocus.current = null
    if (target === null) return
    list.current
      ?.querySelector<HTMLElement>(`[data-card="${target.card}"] [data-control="${target.control}"]`)
      ?.focus({ preventScroll: true })
  }, [arrangement])

  const change = (next: Arrangement, card: ObjectId, control: string): void => {
    refocus.current = { card, control }
    setFeedback(null)
    setArrangement(next)
  }
  const send = (card: ObjectId, to: Destination): void => change(sendTo(arrangement, card, to), card, to)
  const move = (card: ObjectId, step: -1 | 1): void =>
    change(moveWithin(arrangement, card, step), card, step === -1 ? 'up' : 'down')
  const unchanged = isUnchanged(arrangement, looked)
  const words = applyWords(arrangement, looked)
  const apply = (): void => {
    if (unchanged) {
      onSent('The cards stay on top as they are.')
      onClose()
    } else if (session.manual(arrangeAction(arrangement))) {
      onSent(`Sent: ${words}.`)
    } else {
      setFeedback('The command could not be sent; the connection is not open.')
    }
  }
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    event.stopPropagation()
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
      return
    }
    const card = Number(
      event.target instanceof HTMLElement
        ? event.target.closest('[data-card]')?.getAttribute('data-card')
        : NaN,
    )
    if (Number.isNaN(card)) return
    if (event.altKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) {
      event.preventDefault()
      move(card, event.key === 'ArrowUp' ? -1 : 1)
    } else if (!event.altKey && !event.ctrlKey && !event.metaKey) {
      const to = PLACE_KEYS[event.key.toLowerCase()]
      if (to === undefined || event.target instanceof HTMLInputElement) return
      event.preventDefault()
      send(card, to)
    }
  }
  return (
    <div
      id={id}
      role="group"
      aria-label="Look at the top"
      data-slot="look-arrange"
      onKeyDown={onKeyDown}
      className="grid gap-2 px-2 pt-1 pb-2"
    >
      <p className="text-xs text-muted-foreground">Choose where each card goes.</p>
      <div ref={list} className="grid gap-2">
        {DESTINATIONS.map((place) => {
          const cards = arrangement[place.id]
          if (cards.length === 0) return null
          return (
            <section
              key={place.id}
              role="group"
              aria-label={`${place.heading}: ${cards.length}`}
              data-place={place.id}
              className="grid gap-1"
            >
              <h4 className="text-xs font-medium text-muted-foreground" aria-hidden="true">
                {place.heading} <span className="tabular-nums">({cards.length})</span>
              </h4>
              <ol className="grid gap-1">
                {cards.map((card, at) => (
                  <CardLine
                    key={card}
                    card={card}
                    name={nameOf(card)}
                    object={objects[looked.indexOf(card)]}
                    place={place.id}
                    at={at}
                    total={cards.length}
                    onSend={send}
                    onMove={move}
                  />
                ))}
              </ol>
            </section>
          )
        })}
      </div>
      <p data-slot="command-preview" aria-live="polite" className="text-sm">
        {previewWords(arrangement, looked, nameOf)}
      </p>
      <p className="sr-only">{KEYS_HINT}</p>
      <div className="flex flex-wrap gap-1">
        <Button size="sm" data-slot="command-apply" onClick={apply}>
          {words}
        </Button>
        <Button size="sm" variant="outline" onClick={onClose}>
          Cancel
        </Button>
        <Button size="sm" variant="ghost" onClick={onMore}>
          Look at more
        </Button>
      </div>
      {feedback === null ? null : (
        <p role="status" data-slot="command-feedback" className="text-xs text-muted-foreground">
          {feedback}
        </p>
      )}
    </div>
  )
}

interface LineProps {
  card: ObjectId
  name: string
  object: ObjectView | undefined
  place: Destination
  at: number
  total: number
  onSend: (card: ObjectId, to: Destination) => void
  onMove: (card: ObjectId, step: -1 | 1) => void
}

/** One card: its picture and name, the place it goes to and the moves up and down in its list. */
function CardLine({ card, name, object, place, at, total, onSend, onMove }: LineProps) {
  const label = DESTINATIONS.find((option) => option.id === place)?.label ?? ''
  return (
    <li
      aria-label={`${name}, ${label.toLowerCase()}, ${at + 1} of ${total}`}
      data-card={card}
      className="grid gap-1 rounded-md border border-border p-1"
    >
      {/* The moves sit on the name's line, so the three places keep one line in a 280 px column. */}
      <div data-slot="look-card-head" className="flex min-w-0 items-center gap-1">
        <Thumb object={object} />
        <span className="min-w-0 flex-1 truncate text-sm font-medium">
          <span className="tabular-nums text-muted-foreground">{at + 1}.</span> {name}
        </span>
        <Button
          size="icon-sm"
          variant="outline"
          aria-label={`Move ${name} up`}
          aria-disabled={at === 0 || undefined}
          data-control="up"
          onClick={() => onMove(card, -1)}
        >
          <ArrowUp aria-hidden="true" />
        </Button>
        <Button
          size="icon-sm"
          variant="outline"
          aria-label={`Move ${name} down`}
          aria-disabled={at === total - 1 || undefined}
          data-control="down"
          onClick={() => onMove(card, 1)}
        >
          <ArrowDown aria-hidden="true" />
        </Button>
      </div>
      <div role="group" aria-label={`Where ${name} goes`} className="flex flex-wrap gap-1">
        {DESTINATIONS.map((option) => (
          <Button
            key={option.id}
            size="sm"
            variant={option.id === place ? 'default' : 'outline'}
            aria-pressed={option.id === place}
            data-control={option.id}
            onClick={() => onSend(card, option.id)}
            className="px-2"
          >
            {option.label}
          </Button>
        ))}
      </div>
    </li>
  )
}
