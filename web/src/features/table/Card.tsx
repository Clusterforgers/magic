import { memo, useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { cardLabel } from './cardModel'
import type { Highlight } from './cardModel'
import { HIGHLIGHT, SIZE } from './cardSize'
import type { CardSize } from './cardSize'
import { CardFace } from './CardFace'
import { useCreatureOutlook } from './combat/drawn'
import { outlookWords } from './combat/forecastText'
import { ForecastMarks } from './combat/ForecastMarks'
import { FxFloat } from './fx/FxFloat'
import { useCardFx } from './fx/hooks'
import { fxAttr, fxSide } from './fx/model'
import { useCardMark, useMarksStore } from './prompt/marks'
import { combatRoleIn } from './selectors'
import { usePreviewHandlers } from './usePreviewHandlers'

/** Props of a card. */
export interface CardProps {
  /** The object to draw; the card reads it from the store itself, so a parent passes only the id. */
  id: ObjectId
  size?: CardSize
  /** The box (`--card-w` by `--card-h`) comes from the row the card sits in, which fits it to its zone; `size` then only picks the face. */
  fit?: boolean
  /** False takes the card out of the tab order; a row keeps exactly one of its cards tabbable. */
  tabbable?: boolean
  /** Overrides the mark the active prompt gives this card (see `prompt/marks.ts`): legal, dimmed or chosen. */
  highlight?: Highlight
  /** Overrides what the active prompt does on a click: called with the id on a click or an activation by keyboard. */
  onSelect?: (id: ObjectId) => void
}

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
  tabbable = true,
  highlight: given,
  onSelect: givenSelect,
}: CardProps) {
  const object = usePlay((state) => state.objectsById[id])
  // The names of what it is attached to and of what is attached to it, for its cue and its spoken name.
  const hostName = usePlay((state) =>
    object?.attached_to === undefined ? undefined : state.objectsById[object.attached_to]?.name,
  )
  const attachedNames = usePlay(
    useShallow((state) =>
      (object?.attachments ?? []).map((at) => state.objectsById[at]?.name ?? 'a permanent'),
    ),
  )
  const links = useMemo(() => ({ host: hostName, attached: attachedNames }), [hostName, attachedNames])
  const role = usePlay((state) => combatRoleIn(state, id))
  const marks = useMarksStore()
  const { highlight: marked, toggles, note } = useCardMark(id)
  const outlook = useCreatureOutlook(id)
  const fx = useCardFx(id)
  const highlight = given ?? marked
  // An explicit highlight is a toggle, as it always was; a prompt says so when its marks are.
  const pressable = given !== undefined || toggles
  const promptSelects = given === undefined && (marked === 'legal' || marked === 'selected')
  const onSelect = givenSelect ?? (promptSelects ? marks.select : undefined)
  const handlers = usePreviewHandlers(id, onSelect === undefined ? undefined : () => onSelect(id))
  if (object === undefined) return null
  const tapped = object.permanent?.tapped === true
  const markWords =
    highlight === undefined || (pressable && highlight === 'selected') ? '' : `, ${MARK_WORDS[highlight]}`
  const words = note === undefined ? markWords : `${markWords}, ${note.words}`
  return (
    <button
      type="button"
      data-object-id={id}
      data-highlight={highlight}
      data-size={size}
      data-tapped={tapped || undefined}
      data-doomed={outlook?.dies || undefined}
      data-fx={fxAttr(fx)}
      data-fx-side={fxSide(fx)}
      aria-label={cardLabel(object, role, links) + words + outlookWords(outlook)}
      aria-pressed={highlight === undefined || !pressable ? undefined : highlight === 'selected'}
      tabIndex={tabbable ? 0 : -1}
      {...handlers}
      className={cn(
        // The box is the same tapped or not (the face lies down inside it), so a row counts one box per card.
        'relative grid h-(--card-h) w-(--card-w) shrink-0 place-items-center rounded-md outline-offset-2 outline-primary focus-visible:outline-2',
        fit ? null : SIZE[size].vars,
        highlight === undefined ? null : HIGHLIGHT[highlight],
      )}
    >
      <CardFace object={object} role={role} size={size} fit={fit} note={note} links={links} />
      {outlook === null ? null : (
        <ForecastMarks outlook={outlook} tapped={tapped} compact={size === 'compact'} />
      )}
      {fx.map((entry) =>
        entry.kind === 'damaged' ? <FxFloat key={entry.token} kind="damaged" amount={entry.amount} /> : null,
      )}
    </button>
  )
})
