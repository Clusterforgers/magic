import { useMemo } from 'react'
import type { ObjectId } from '@/protocol/generated'
import { useBoardBadges } from './boardBadges'
import type { BadgeTone, BoardBadge } from './boardBadges'
import { useAsker } from './useAsker'
import { usePlay } from '@/features/game/gameContext'
import { usePreviewTarget } from '../../preview'
import { stackLights } from '../../stackHover'
import { useStackEntries } from '../../useStackEntries'

/** The colour of a tone and of the words on it. */
const TONE: Record<BadgeTone, { edge: string; ink: string }> = {
  ask: { edge: 'var(--turn-you)', ink: 'var(--on-turn-you)' },
  cause: { edge: 'var(--success)', ink: 'var(--surface)' },
  placed: { edge: 'var(--selected)', ink: 'var(--on-selected)' },
  done: { edge: 'var(--success)', ink: 'var(--surface)' },
  proposed: { edge: 'var(--primary)', ink: 'var(--primary-foreground)' },
}

/** A table card by id: the board's and the hand's cards carry `data-size`, a stack chip does not. */
const card = (id: ObjectId): string => `[data-slot='table-grid'] button[data-object-id='${id}'][data-size]`

/** Words safe inside a CSS string. */
const quoted = (words: string): string => `'${words.replace(/[\\']/g, '').replace(/[\n\r]/g, ' ')}'`

/** The style rules of one badge: a ring in its tone and its word in the card's lower left corner, where no board edge cuts it. */
function badgeRules({ id, label, tone }: BoardBadge): string {
  const { edge, ink } = TONE[tone]
  const glow =
    tone === 'ask'
      ? `box-shadow:0 0 0 5px color-mix(in oklch,${edge} 32%,transparent),0 0 22px color-mix(in oklch,${edge} 55%,transparent);`
      : ''
  // A source a proposed payment taps: a dashed edge and turned a little, as a tapped card leans.
  const style = tone === 'proposed' ? 'dashed' : 'solid'
  const turn = tone === 'proposed' ? 'rotate:-8deg;' : ''
  return (
    `${card(id)}{outline:2.5px ${style} ${edge};outline-offset:1px;overflow:visible;z-index:3;${turn}${glow}}` +
    `${card(id)}::after{content:${quoted(label)};position:absolute;bottom:0.1875rem;left:0.1875rem;z-index:4;max-width:calc(100% - 0.375rem);overflow:hidden;text-overflow:ellipsis;` +
    `padding:0 0.5rem;border-radius:999px;background:${edge};color:${ink};font-weight:700;font-size:0.75rem;line-height:1rem;` +
    `white-space:nowrap;pointer-events:none;box-shadow:0 1px 3px oklch(0 0 0/.4)}`
  )
}

/** The rule of the card a chip or row points at: a stronger ring that pulses unless motion is reduced. */
function litRules(id: ObjectId): string {
  return (
    `${card(id)}{outline:3px solid var(--turn-you);outline-offset:2px;z-index:3;` +
    `box-shadow:0 0 0 7px color-mix(in oklch,var(--turn-you) 40%,transparent),0 0 28px var(--turn-you)}` +
    `@media (prefers-reduced-motion:no-preference){${card(id)}{animation:ask-lit 900ms var(--ease-out) infinite alternate}}` +
    `@keyframes ask-lit{from{outline-offset:1px}to{outline-offset:4px}}`
  )
}

/**
 * Lights the cards a decision is about, as style rules by object id, so no card redraws: the asking card glows with an
 * "asking" badge, what triggered it has "just entered" (or "cause"), a prompt's own badges (the numbers of triggers
 * being ordered) and the card a hovered chip or row points at. Decoration: the band and the column say the same in words.
 */
export function AskMarks() {
  const asker = useAsker()
  const { badges, lit } = useBoardBadges()
  const shown = usePreviewTarget()?.id ?? null
  const entries = useStackEntries()
  const objectsById = usePlay((state) => state.objectsById)
  const css = useMemo(() => {
    const all: BoardBadge[] = []
    const taken = new Set<ObjectId>()
    const add = (badge: BoardBadge): void => {
      if (taken.has(badge.id)) return
      taken.add(badge.id)
      all.push(badge)
    }
    for (const badge of badges) add(badge)
    if (asker?.board !== undefined) add({ id: asker.board, label: 'asking', tone: 'ask' })
    if (asker?.cause !== undefined) {
      add({
        id: asker.cause.object,
        label: asker.cause.entered === true ? 'just entered' : 'cause',
        tone: 'cause',
      })
    }
    // A pointed stack item lights its source and its targets too (`stackHover.ts`), in every decision and none.
    const pointed = new Set([...(lit === null ? [] : [lit]), ...stackLights(shown, objectsById, entries)])
    return all.map(badgeRules).join('') + [...pointed].map(litRules).join('')
  }, [asker, badges, lit, shown, objectsById, entries])
  if (css === '') return null
  return <style data-slot="ask-marks">{css}</style>
}
