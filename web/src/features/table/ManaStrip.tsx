import { useMemo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { foldGives, givesNow, manaAbilitiesOf } from './manaMade'
import { ManaMade } from './ManaMade'

// The symbol and the glyph in it scale with the card (`--card-w`); the glyph is part of the picture, not text to read.
const SYMBOL =
  'size-[clamp(8px,calc(var(--card-w)*0.09),13px)] text-[length:clamp(4px,calc(var(--card-w)*0.045),7px)]'

/** The preview's line for a permanent that taps for mana: "Taps for" and the symbols, in words to a screen reader. */
export function ManaPreviewLine({ object }: { object: ObjectView }) {
  const gives = useMemo(() => foldGives(manaAbilitiesOf(object).filter((ability) => ability.taps)), [object])
  if (gives === null || object.zone !== 'Battlefield' || (!gives.any && gives.ways.length === 0)) return null
  return (
    <p data-slot="preview-mana" className="flex flex-wrap items-center gap-x-1.5">
      <span>Taps for</span>
      <ManaMade gives={gives} sizeClass="size-4 text-[0.6875rem]" words />
    </p>
  )
}

/**
 * What one tap of a card gives, as a few small symbols in its bottom left corner (the artist line of a picture, clear
 * of the art and the rules text): only on a permanent the seat may tap for mana now, so a tapped land, an opponent's
 * land and a card in a hand draw nothing. Decoration without a pointer; the preview says the same in words.
 */
export function ManaStrip({ id }: { id: ObjectId }) {
  const object = usePlay((state) => state.objectsById[id])
  const seat = usePlay((state) => state.seat)
  const gives = useMemo(() => (object === undefined ? null : givesNow(object, seat)), [object, seat])
  if (gives === null) return null
  return (
    <span
      aria-hidden="true"
      data-slot="mana-strip"
      className="pointer-events-none absolute bottom-[1.5%] left-[3%] z-10 flex max-w-[55%] overflow-hidden rounded-full bg-background/75 px-0.5 py-px"
    >
      <ManaMade gives={gives} sizeClass={SYMBOL} className="flex-nowrap gap-x-0.5" />
    </span>
  )
}
