/**
 * A card that is not on Scryfall (a relic, a boss card, a boss phase, the Spirit's haunt), drawn as a framed card:
 * name, an art panel tinted by what the card is for, the type line, the rules text and a relic's role. A risky relic,
 * whose effect helps the boss too, wears a hazard edge and a "Dangerous" badge.
 */
import { CrownIcon, FlameIcon, GemIcon, GhostIcon, SwordsIcon, TriangleAlertIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'
import type { CardKind, CustomCardView } from '@/protocol/generated'
import { isRisky } from './model'
import { roleLabel } from './words'

const ART: Record<CardKind, { icon: LucideIcon; tint: string; edge: string }> = {
  Relic: { icon: GemIcon, tint: 'from-relic/25 to-relic/60', edge: 'border-relic' },
  Boss: { icon: CrownIcon, tint: 'from-delve/25 to-delve/70', edge: 'border-delve' },
  Phase: { icon: FlameIcon, tint: 'from-destructive/20 to-delve/60', edge: 'border-delve' },
  Haunt: { icon: GhostIcon, tint: 'from-turn-them/20 to-turn-them/60', edge: 'border-turn-them' },
}

/** The badge of a risky relic: a warning icon and "Dangerous", with what makes it so for a screen reader. */
export function RiskBadge({ className }: { className?: string }) {
  return (
    <span
      data-slot="risk-badge"
      className={cn(
        'inline-flex items-center gap-1 rounded-full border border-destructive bg-surface px-2 text-xs font-semibold text-destructive',
        className,
      )}
    >
      <TriangleAlertIcon aria-hidden="true" className="size-3.5" />
      Dangerous
      <span className="sr-only">: it helps the boss too</span>
    </span>
  )
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
  const risky = isRisky(card)
  return (
    <figure
      data-slot="custom-card"
      data-kind={card.kind}
      data-risky={risky || undefined}
      aria-label={card.name}
      className={cn(
        'flex w-full flex-col sm:w-56 overflow-hidden rounded-xl border-2 bg-card shadow-e1',
        risky ? 'border-destructive' : (art?.edge ?? 'border-border-strong'),
        className,
      )}
    >
      <figcaption className="flex items-center justify-between gap-2 border-b px-3 py-1 text-sm font-semibold leading-tight">
        <span className="min-w-0">{card.name}</span>
        {card.kind === 'Relic' && card.price > 0 ? (
          <span className="shrink-0 text-xs font-medium text-relic">{card.price} g</span>
        ) : null}
      </figcaption>
      <div
        role="img"
        aria-label={`Art placeholder: ${card.art || card.kind}`}
        className={cn(
          'relative flex h-24 items-center justify-center bg-linear-to-br',
          risky ? 'from-destructive/15 to-destructive/45' : art?.tint,
        )}
      >
        {risky ? (
          <span
            aria-hidden="true"
            className="absolute inset-x-0 top-0 h-2 bg-[repeating-linear-gradient(135deg,var(--destructive)_0_8px,transparent_8px_16px)]"
          />
        ) : null}
        <Icon aria-hidden="true" className="size-10 text-foreground/80 drop-shadow-sm" />
        {risky ? <RiskBadge className="absolute bottom-2 left-2" /> : null}
      </div>
      <div className="flex items-center justify-between gap-2 border-b px-3 py-1 text-xs font-medium">
        <span>{card.type_line}</span>
        {card.role !== '' && !risky ? (
          <span className="rounded px-1 text-muted-foreground">{roleLabel(card.role)}</span>
        ) : null}
      </div>
      <p className="min-h-16 flex-1 whitespace-pre-line px-3 py-2 text-xs leading-snug">{card.text}</p>
      {risky ? (
        <p className="border-t border-destructive/40 px-3 py-1 text-xs font-medium text-destructive">
          {roleLabel(card.role)}
        </p>
      ) : null}
      {children ? <div className="flex gap-2 border-t p-2">{children}</div> : null}
    </figure>
  )
}
