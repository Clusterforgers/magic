import { memo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { cardLabel } from './cardModel'
import type { Highlight } from './cardModel'
import { HIGHLIGHT, PICKED, SIZE } from './cardSize'
import type { CardSize } from './cardSize'
import { CardFace } from './CardFace'
import { useCreatureOutlook } from './combat/drawn'
import { outlookWords } from './combat/forecastText'
import { ForecastMarks } from './combat/ForecastMarks'
import { FxFloat } from './fx/FxFloat'
import { FxMark } from './fx/FxMark'
import { useCardFx } from './fx/hooks'
import { markShowsWord } from './fx/marks'
import { fxAttr, fxSide, isMarkFx } from './fx/model'
import { useDrawnWidth } from './ImageFace'
import { ManaStrip } from './ManaStrip'
import { BY_HAND_KEY, clausesOf } from './manual/clauses'
import { useIsPicked } from './manual/panel/pick'
import { useManualMenu } from './manual/useManualMenu'
import { useCardMark, useMarksStore } from './prompt/marks'
import type { CardSelect } from './prompt/marks'
import { handMarkWords, revealedPlace } from './revealed'
import { combatRoleIn } from './selectors'
import { useLinks } from './useLinks'
import { usePreviewHandlers } from './usePreviewHandlers'

/** Props of a card. */
export interface CardProps {
  /** The object to draw; the card reads it from the store itself, so a parent passes only the id. */
  id: ObjectId
  size?: CardSize
  /** The box (`--card-w` by `--card-h`) comes from the row the card sits in, which fits it to its zone; `size` then only picks the face. */
  fit?: boolean
  /** A compact face still draws the mana cost: the hand, where the cost decides what can be cast. */
  cost?: boolean
  /** False takes the card out of the tab order; a row keeps exactly one of its cards tabbable. */
  tabbable?: boolean
  /** Overrides the mark the active prompt gives this card (see `prompt/marks.ts`): legal, dimmed or chosen. */
  highlight?: Highlight
  /** Overrides what the active prompt does on a click: called with the id on a click or an activation by keyboard. */
  onSelect?: CardSelect
  /** Words added to the spoken name, such as where the card waits and what casting it costs. */
  words?: string
}

/** What the picked mark adds to a card's spoken name. */
const PICKED_WORDS = 'picked in the Commands panel'

/** What a mark adds to a card's spoken name; a selection that reads as pressed needs no extra word. */
const MARK_WORDS: Record<Highlight, string> = {
  legal: 'available',
  dimmed: 'not available',
  selected: 'selected',
}

/**
 * One card of the table. It subscribes to its own object, combat role and prompt mark only and is memoised, so a
 * batch redraws the cards it changed and no others. Hover and focus preview it; a click pins the preview and, on a
 * legal or selected card, tells the active prompt.
 */
export const Card = memo(function Card({
  id,
  size = 'normal',
  fit = false,
  cost = false,
  tabbable = true,
  highlight: given,
  onSelect: givenSelect,
  words: extra,
}: CardProps) {
  const object = usePlay((state) => state.objectsById[id])
  // The names of what it is attached to, copies or shows the front of, and of what is attached to it.
  const links = useLinks(object)
  const role = usePlay((state) => combatRoleIn(state, id))
  // Face up to every seat: a string or nothing, so the selector needs no `useShallow`.
  const place = usePlay((state) => (state.view === null ? undefined : revealedPlace(state.view.players, id)))
  const marks = useMarksStore()
  const { highlight: marked, toggles, note } = useCardMark(id)
  const outlook = useCreatureOutlook(id)
  const fx = useCardFx(id)
  // A mark that cannot write its word on a card this wide shows its icon alone.
  const drawnWidth = useDrawnWidth(size, fit)
  const highlight = given ?? marked
  // An explicit highlight is a toggle, as it always was; a prompt says so when its marks are.
  const pressable = given !== undefined || toggles
  const promptSelects = given === undefined && (marked === 'legal' || marked === 'selected')
  const onSelect = givenSelect ?? (promptSelects ? marks.select : undefined)
  const handlers = usePreviewHandlers(id, onSelect === undefined ? undefined : (from) => onSelect(id, from))
  const manual = useManualMenu(id)
  const picked = useIsPicked(id)
  if (object === undefined) return null
  const tapped = object.permanent?.tapped === true
  const attached = object.attached_to !== undefined || object.attached_player !== undefined
  // The key that lists the clauses left to the table is the card's own, so its spoken name says so.
  const listKey = manual.onKeyDown !== undefined && clausesOf(object).length > 0 ? BY_HAND_KEY : undefined
  const markWords =
    highlight === undefined || (pressable && highlight === 'selected') ? '' : `, ${MARK_WORDS[highlight]}`
  const spoken = [
    ...(note === undefined ? [] : [note.words]),
    ...handMarkWords(place, object.note),
    ...(picked ? [PICKED_WORDS] : []),
  ]
  const words = spoken.length === 0 ? markWords : `${markWords}, ${spoken.join(', ')}`
  return (
    <button
      type="button"
      data-object-id={id}
      data-highlight={highlight}
      data-cue={note?.cue}
      data-picked={picked || undefined}
      data-size={size}
      data-tapped={tapped || undefined}
      data-attached={attached || undefined}
      data-hosting={object.attachments === undefined ? undefined : object.attachments.length}
      data-doomed={outlook?.dies || undefined}
      data-fx={fxAttr(fx)}
      data-fx-side={fxSide(fx)}
      aria-label={
        cardLabel(object, role, links, listKey?.toUpperCase()) +
        (extra === undefined ? '' : `, ${extra}`) +
        words +
        outlookWords(outlook)
      }
      aria-keyshortcuts={listKey}
      aria-pressed={highlight === undefined || !pressable ? undefined : highlight === 'selected'}
      tabIndex={tabbable ? 0 : -1}
      {...handlers}
      {...manual}
      className={cn(
        // The box is the same tapped or not (the face lies down inside it), so a row counts one box per card.
        'group/card relative grid h-(--card-h) w-(--card-w) shrink-0 place-items-center rounded-md outline-offset-2 outline-primary focus-visible:outline-2',
        fit ? null : SIZE[size].vars,
        highlight === undefined ? null : HIGHLIGHT[highlight],
        picked ? PICKED : null,
        // A card that says why it is dimmed is read whole while it says so.
        note?.hint === undefined
          ? null
          : 'hover:opacity-100 hover:saturate-100 focus-visible:opacity-100 focus-visible:saturate-100',
      )}
    >
      <CardFace
        object={object}
        role={role}
        size={size}
        fit={fit}
        cost={cost}
        note={note}
        links={links}
        revealed={place !== undefined}
        handNote={object.note}
      />
      {outlook === null ? null : (
        <ForecastMarks outlook={outlook} tapped={tapped} compact={size === 'compact'} />
      )}
      {fx.map((entry) =>
        entry.kind === 'damaged' ? <FxFloat key={entry.token} kind="damaged" amount={entry.amount} /> : null,
      )}
      {fx.map((entry) =>
        isMarkFx(entry.kind) ? (
          <FxMark key={entry.token} kind={entry.kind} compact={!markShowsWord(entry.kind, drawnWidth)} />
        ) : null,
      )}
      <ManaStrip id={id} />
      {note?.hint === undefined ? null : (
        <span
          aria-hidden="true"
          data-slot="card-hint"
          // Wraps on a narrow card ("Needs" over "mana") rather than ending in an ellipsis.
          className="pointer-events-none absolute inset-x-0.5 bottom-0.5 z-10 hidden rounded-sm bg-background/95 px-0.5 text-center text-xs leading-4 font-semibold [overflow-wrap:anywhere] text-foreground shadow-sm ring-1 ring-foreground/20 group-hover/card:block group-focus-visible/card:block"
        >
          {note.hint}
        </span>
      )}
    </button>
  )
})
