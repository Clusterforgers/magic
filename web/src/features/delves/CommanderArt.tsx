/**
 * The look of a deck or a boss in the Delves pages: a commander's art crop (two side by side for partners), else a
 * ground in the deck's colours with an icon, and the colours as mana symbols.
 */
import { CrownIcon, LayersIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useState } from 'react'
import { faceCosts } from '@/features/builder/manaText'
import { cardImageUrl, identityOf } from '@/features/cards/images'
import { ManaPips } from '@/features/table/ManaPips'
import { cn } from '@/lib/utils'
import { colourGround } from './art'
import { COLOUR_NAME } from './deckPicker'

/** A card whose art stands for a deck. */
export interface ArtCard {
  oracleId: string
  name: string
  printingId?: string
}

function Crop({ card, onMiss }: { card: ArtCard; onMiss: () => void }) {
  return (
    <img
      src={cardImageUrl(
        { kind: 'card', identity: identityOf(card.oracleId, card.printingId ?? null) },
        0,
        'art_crop',
      )}
      alt=""
      loading="lazy"
      decoding="async"
      onError={onMiss}
      className="size-full min-w-0 flex-1 object-cover"
    />
  )
}

/** The art of a deck's commanders filling its box; the colour ground with an icon where there is none or it fails. */
export function CommanderArt({
  cards,
  colours,
  icon: Icon = LayersIcon,
  className,
  label,
}: {
  cards: readonly ArtCard[]
  colours: string
  icon?: LucideIcon
  className?: string
  /** Names the picture for a screen reader; the picture is decoration without it. */
  label?: string
}) {
  const [missed, setMissed] = useState(false)
  const shown = missed ? [] : cards.slice(0, 2)
  return (
    <div
      data-slot="commander-art"
      role={label === undefined ? undefined : 'img'}
      aria-label={label}
      aria-hidden={label === undefined ? true : undefined}
      className={cn('relative flex overflow-hidden bg-linear-to-br', colourGround(colours), className)}
    >
      {shown.length > 0 ? (
        shown.map((card) => <Crop key={card.oracleId} card={card} onMiss={() => setMissed(true)} />)
      ) : (
        <span className="grid size-full place-items-center">
          <Icon aria-hidden="true" className="size-8 text-foreground/70 drop-shadow-sm" />
        </span>
      )}
    </div>
  )
}

/** A boss's portrait: its colours with a crown, as the content has no art for it yet. */
export function BossPortrait({ colours, className }: { colours: string; className?: string }) {
  return <CommanderArt cards={[]} colours={colours} icon={CrownIcon} className={className} />
}

/** The colours of a deck or boss as mana symbols, spoken by the symbols' names; colourless as {C}. */
export function ColourPips({ colours, size = 'sm' }: { colours: string; size?: 'sm' | 'md' }) {
  const letters = [...colours].filter((c) => c in COLOUR_NAME)
  const cost = letters.length === 0 ? '{C}' : letters.map((c) => `{${c}}`).join('')
  const pips = faceCosts(cost)[0] ?? []
  return (
    <ManaPips pips={pips} sizeClass={size === 'md' ? 'size-5 text-xs' : 'size-4 text-label'} label="Colors" />
  )
}
