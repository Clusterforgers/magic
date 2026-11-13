/**
 * A card that is not on Scryfall (a relic, a boss card, a boss phase, a scheme, the Spirit's haunt), drawn as a text frame:
 * name, an art placeholder tinted by what the card is for, the type line, the rules text and a relic's role.
 */
import { CrownIcon, EyeIcon, FlameIcon, GemIcon, GhostIcon, SwordsIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'
import type { CardKind, CustomCardView } from '@/protocol/generated'
import { roleLabel } from './words'

const ART: Record<CardKind, { icon: LucideIcon; tint: string }> = {
  Relic: { icon: GemIcon, tint: 'from-amber-200 to-amber-500 dark:from-amber-700 dark:to-amber-950' },
  Boss: { icon: CrownIcon, tint: 'from-rose-300 to-rose-700 dark:from-rose-800 dark:to-rose-950' },
  Phase: { icon: FlameIcon, tint: 'from-orange-300 to-red-600 dark:from-orange-800 dark:to-red-950' },
  Haunt: { icon: GhostIcon, tint: 'from-sky-200 to-indigo-500 dark:from-sky-800 dark:to-indigo-950' },
  Scheme: { icon: EyeIcon, tint: 'from-violet-300 to-purple-700 dark:from-violet-800 dark:to-purple-950' },
}

export function CustomCardFrame({
  card,
  className,
  children,
}: {
  card: CustomCardView
  className?: string
  /** Controls under the card, such as a Take button. */
  children?: ReactNode
}) {
  const art = ART[card.kind]
  const Icon = art?.icon ?? SwordsIcon
  return (
    <figure
      data-slot="custom-card"
      data-kind={card.kind}
      aria-label={card.name}
      className={cn(
        'flex w-56 flex-col overflow-hidden rounded-xl border-2 border-foreground/70 bg-card shadow-sm',
        className,
      )}
    >
      <figcaption className="border-b px-3 py-1.5 text-sm font-semibold leading-tight">
        {card.name}
      </figcaption>
      <div
        role="img"
        aria-label={`Art placeholder: ${card.art || card.kind}`}
        className={cn('flex h-24 items-center justify-center bg-gradient-to-br', art?.tint)}
      >
        <Icon aria-hidden="true" className="size-10 text-white/90 drop-shadow" />
      </div>
      <div className="flex items-center justify-between gap-2 border-y px-3 py-1 text-xs font-medium">
        <span>{card.type_line}</span>
        {card.role !== '' ? (
          <span
            className={cn(
              'rounded px-1',
              card.role === 'Risky' ? 'text-destructive' : 'text-muted-foreground',
            )}
          >
            {roleLabel(card.role)}
          </span>
        ) : null}
      </div>
      <p className="min-h-16 flex-1 whitespace-pre-line px-3 py-2 text-xs leading-snug">{card.text}</p>
      {children ? <div className="flex gap-2 border-t p-2">{children}</div> : null}
    </figure>
  )
}
