import { ChevronLeft, ChevronRight, ChevronsDown, ChevronsUp } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { DragEvent, KeyboardEvent } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { usePreviewHandlers } from '../../usePreviewHandlers'
import { cardPlaceWords } from '../cardsText'
import { askerWords, numberKey } from '../choiceText'
import { uniqueNames } from '../combatText'
import {
  pileOfCard,
  crossed,
  isStart,
  placed,
  scryKeys,
  scryPreview,
  shifted,
  startPiles,
} from '../scryPiles'
import type { PileName, Piles } from '../scryPiles'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import { PromptPicture } from '../PromptPicture'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'
import { usePromptKeys } from '../usePromptKeys'

/** Up to this many cards are drawn with their cost and type line; more use the compact face. */
const NORMAL_UP_TO = 3

/** The arrow keys that move the focused card toward the top (-1) or the bottom (1) of its pile. */
const ARROWS: Readonly<Record<string, -1 | 1>> = { ArrowUp: -1, ArrowLeft: -1, ArrowDown: 1, ArrowRight: 1 }

const HEADING: Record<PileName, string> = { top: 'Keep on top', bottom: 'Put on the bottom' }

interface CardProps {
  id: ObjectId
  object: ObjectView | undefined
  label: string
  pile: PileName
  place: number
  count: number
  size: 'normal' | 'compact'
  onMove(id: ObjectId, by: -1 | 1): void
  onCross(id: ObjectId): void
  onDragStart(id: ObjectId): void
  onDropBefore(before: ObjectId, pile: PileName): void
}

/** One card of a pile: its picture (draggable) and the buttons that move it within the pile or to the other one. */
function PileCard({
  id,
  object,
  label,
  pile,
  place,
  count,
  size,
  onMove,
  onCross,
  onDragStart,
  onDropBefore,
}: CardProps) {
  const preview = usePreviewHandlers(id)
  const other = pile === 'top' ? 'the bottom' : 'the top'
  return (
    <li
      data-card={id}
      draggable
      onDragStart={(event: DragEvent<HTMLElement>) => {
        event.dataTransfer.setData('text/plain', String(id))
        event.dataTransfer.effectAllowed = 'move'
        onDragStart(id)
      }}
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault()
        event.stopPropagation()
        onDropBefore(id, pile)
      }}
      onFocus={preview.onFocus}
      onBlur={preview.onBlur}
      className="flex min-w-0 cursor-grab flex-col items-center gap-1"
    >
      <span className="text-xs tabular-nums">
        <span className="text-muted-foreground">{place + 1}.</span>
        <span className="sr-only"> {label}</span>
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
          onClick={() => onMove(id, -1)}
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
          onClick={() => onMove(id, 1)}
        >
          <ChevronRight aria-hidden="true" />
        </Button>
        <Button
          size="icon-xs"
          variant="outline"
          data-card={id}
          data-move="cross"
          aria-label={`Send ${label} to ${other}`}
          title={`Send to ${other}`}
          onClick={() => onCross(id)}
        >
          {pile === 'top' ? <ChevronsDown aria-hidden="true" /> : <ChevronsUp aria-hidden="true" />}
        </Button>
      </span>
    </li>
  )
}

/**
 * Scry: the cards looked at, as pictures numbered from the top of the library, in two piles, the cards that stay on
 * top and the cards that go to the bottom, each in the order it will lie (the card nearest the top first, on the
 * left). Cards are dragged between the piles and into place; the buttons under a card move it within its pile or to
 * the other one, Alt with the arrow keys moves the focused card and a number key sends the card looked at in that place
 * to the other pile. A line under the piles gives the result. Confirm sends both piles as one answer. The server's
 * answer at the deadline keeps every card on top in the order looked at.
 */
export const ScryPrompt: PromptComponent<'Scry'> = ({ options }) => {
  const { answer } = usePrompt()
  const looked = options.cards
  const known = useObjects(useMemo(() => [options.source, ...looked], [options.source, looked]))
  const objects = useMemo(
    () => new Map(looked.map((id, place) => [id, known[place + 1]] as const)),
    [looked, known],
  )
  const labels = useMemo(
    () => uniqueNames(looked.map((id) => ({ id, name: objects.get(id)?.name ?? 'A card' }))),
    [looked, objects],
  )
  const [piles, setPiles] = useState<Piles>(() => startPiles(looked))
  const [note, setNote] = useState<string | null>(null)
  const dragged = useRef<ObjectId | null>(null)
  const pressed = useRef<{ id: ObjectId; move: string } | null>(null)
  const root = useRef<HTMLDivElement>(null)
  const nameOf = (id: ObjectId): string => labels.get(id) ?? 'A card'

  const apply = (next: Piles, id: ObjectId, move: string): void => {
    pressed.current = { id, move }
    setPiles(next)
    const pile = pileOfCard(next, id)
    if (pile === null) return
    const list = next[pile]
    setNote(
      `${nameOf(id)} is ${pile === 'top' ? 'on top' : 'on the bottom'}, ${cardPlaceWords(list.indexOf(id), list.length)}.`,
    )
  }
  const move = (id: ObjectId, by: -1 | 1): void =>
    apply(shifted(piles, id, by), id, by === -1 ? 'up' : 'down')
  const cross = (id: ObjectId): void => apply(crossed(piles, id), id, 'cross')
  const dropped = (before: ObjectId | null, pile: PileName): void => {
    const id = dragged.current
    dragged.current = null
    if (id !== null) apply(placed(piles, id, pile, before), id, 'cross')
  }
  // A moved card is a new place in the page, which can drop focus; it goes back to the button just pressed.
  useEffect(() => {
    const last = pressed.current
    pressed.current = null
    if (last === null) return
    root.current
      ?.querySelector<HTMLButtonElement>(`button[data-card="${last.id}"][data-move="${last.move}"]`)
      ?.focus()
  }, [piles])
  const onKeyDown = (event: KeyboardEvent<HTMLElement>): void => {
    if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return
    const by = ARROWS[event.key]
    if (by === undefined || !(event.target instanceof Element)) return
    const raw = event.target.closest('[data-card]')?.getAttribute('data-card')
    if (raw === null || raw === undefined) return
    event.preventDefault()
    move(Number(raw), by)
  }
  usePromptKeys(
    Object.fromEntries(looked.slice(0, 10).map((id, place) => [numberKey(place) ?? '', () => cross(id)])),
  )

  const asker = askerWords(known[0]?.name)
  const words = options.text ?? ''
  const size = looked.length <= NORMAL_UP_TO ? 'normal' : 'compact'
  const total = looked.length
  const sentence = `${asker}: ${words === '' ? 'Scry: choose which cards go to the bottom and put the piles in order.' : words}`
  const pile = (name: PileName) => (
    <div
      key={name}
      role="group"
      aria-label={`${HEADING[name]}: ${piles[name].length}`}
      data-slot={`scry-${name}`}
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault()
        dropped(null, name)
      }}
      className={cn(
        'flex min-w-0 max-w-full grow flex-col gap-1 rounded-md border border-border p-1.5',
        name === 'bottom' && 'bg-muted/40',
      )}
    >
      <p className="text-xs font-medium text-muted-foreground" aria-hidden="true">
        {HEADING[name]} <span className="tabular-nums">({piles[name].length})</span>
      </p>
      {piles[name].length === 0 ? (
        <p data-slot="pile-empty" className="p-2 text-xs text-muted-foreground">
          No cards
        </p>
      ) : (
        <ol
          aria-label={`${HEADING[name]}, the first nearest the top of your library`}
          className="flex flex-wrap content-start gap-2"
        >
          {piles[name].map((id, place) => (
            <PileCard
              key={id}
              id={id}
              object={objects.get(id)}
              label={nameOf(id)}
              pile={name}
              place={place}
              count={piles[name].length}
              size={size}
              onMove={move}
              onCross={cross}
              onDragStart={(card) => (dragged.current = card)}
              onDropBefore={dropped}
            />
          ))}
        </ol>
      )}
    </div>
  )
  return (
    <PromptLayout
      text={
        <>
          {note === null ? sentence : `${asker}: ${note}`}
          <span className="sr-only"> {scryKeys(total)}</span>
        </>
      }
    >
      <Button
        className="h-9 px-4"
        onClick={() => answer({ Scry: { top: [...piles.top], bottom: [...piles.bottom] } })}
      >
        Confirm
      </Button>
      <Button
        variant="outline"
        className="h-9 px-4"
        disabled={isStart(piles, looked)}
        onClick={() => {
          setPiles(startPiles(looked))
          setNote(null)
        }}
      >
        Original order
      </Button>
      <p
        data-slot="scry-preview"
        aria-hidden="true"
        className="line-clamp-2 min-w-0 basis-full text-xs text-muted-foreground"
      >
        {scryPreview(piles.top.map(nameOf), piles.bottom.map(nameOf))}
      </p>
      <div
        ref={root}
        data-slot="scry-piles"
        onKeyDown={onKeyDown}
        className="flex basis-full flex-wrap gap-2 overflow-y-auto max-h-[max(6rem,calc(var(--sheet-max,20rem)_-_10.5rem))]"
      >
        {pile('top')}
        {pile('bottom')}
      </div>
    </PromptLayout>
  )
}
