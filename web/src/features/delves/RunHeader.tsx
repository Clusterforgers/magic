/**
 * The head of a run page: the theme, where the run is, Heat, the party's hearts, the seed code to copy, and each hero
 * with a life bar, gold and a row of relics whose rules show on hover or focus.
 */
import { CoinsIcon, FlameIcon, GemIcon, GhostIcon, HeartIcon, TriangleAlertIcon } from 'lucide-react'
import { CopyButton } from '@/components/CopyButton'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'
import type { CustomCardView, DelveContent, DelveView, Hero } from '@/protocol/generated'
import { ColourPips } from './CommanderArt'
import { HEARTS, lifeShare, lifeTone, runStage } from './model'

const BAR = { low: 'bg-destructive', hurt: 'bg-warning', fine: 'bg-success' } as const

/** A hero's life as a bar with the number. */
export function LifeBar({ hero, className }: { hero: Hero; className?: string }) {
  const tone = lifeTone(hero.life)
  return (
    <span
      role="img"
      aria-label={`${hero.name}: ${hero.life} life`}
      className={cn('flex min-w-0 items-center gap-2', className)}
    >
      <span className="h-2 min-w-12 flex-1 overflow-hidden rounded-full bg-muted">
        <span
          data-tone={tone}
          className={cn('block h-full rounded-full transition-[width] duration-slow ease-out', BAR[tone])}
          style={{ width: `${lifeShare(hero.life) * 100}%` }}
        />
      </span>
      <span className="w-7 text-right text-sm font-semibold tabular-nums">{hero.life}</span>
    </span>
  )
}

/** The party's hearts: filled for each left, hollow for each lost. */
export function Hearts({ left, total = HEARTS }: { left: number; total?: number }) {
  return (
    <span role="img" aria-label={`${left} of ${total} hearts left`} className="flex items-center gap-1">
      {Array.from({ length: total }, (_, i) => (
        <HeartIcon
          key={i}
          aria-hidden="true"
          className={cn('size-5', i < left ? 'fill-destructive text-destructive' : 'text-muted-foreground')}
        />
      ))}
    </span>
  )
}

/** A relic as a small gem chip whose name and rules show in a tooltip; a risky one wears a warning. */
export function RelicChip({ card, owner }: { card: CustomCardView; owner?: string }) {
  const risky = card.role === 'Risky'
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            data-relic={card.id}
            data-risky={risky || undefined}
            aria-label={`${card.name}${risky ? ', risky' : ''}${owner ? `, relic of ${owner}` : ''}: ${card.text}`}
            className={cn(
              'inline-grid size-7 place-items-center rounded-md border bg-surface',
              risky ? 'border-destructive text-destructive' : 'border-relic text-relic',
            )}
          />
        }
      >
        {risky ? (
          <TriangleAlertIcon aria-hidden="true" className="size-4" />
        ) : (
          <GemIcon aria-hidden="true" className="size-4" />
        )}
      </TooltipTrigger>
      <TooltipContent className="grid max-w-64 gap-1 py-2">
        <span className="font-semibold">
          {card.name}
          {owner ? ` · ${owner}` : ''}
        </span>
        <span>{card.text}</span>
        {risky ? <span className="font-semibold">Risky: it helps the boss too.</span> : null}
      </TooltipContent>
    </Tooltip>
  )
}

const relicCard = (content: DelveContent, id: string): CustomCardView =>
  content.cards.find((c) => c.id === id) ?? {
    id,
    name: id,
    kind: 'Relic',
    type_line: 'Relic',
    text: '',
    art: '',
    role: '',
    price: 0,
  }

/** One hero of the header: name, gold, life bar, colors and relics. */
function HeroLine({ hero, you, content }: { hero: Hero; you: boolean; content: DelveContent }) {
  return (
    <li data-hero={hero.name} className="grid min-w-0 content-start gap-2 rounded-lg border bg-surface p-2">
      <span className="flex min-w-0 items-center gap-1 text-sm">
        <span className="min-w-0 truncate font-medium">{hero.name}</span>
        {you ? <span className="text-xs text-muted-foreground">(you)</span> : null}
        {hero.life <= 0 ? <GhostIcon aria-label="Fallen" className="size-4 text-muted-foreground" /> : null}
        <span className="ml-auto flex shrink-0 items-center gap-1 text-xs text-muted-foreground">
          <CoinsIcon aria-hidden="true" className="size-3.5 text-relic" />
          <span className="tabular-nums">{hero.gold}</span>
          <span className="sr-only">gold</span>
        </span>
      </span>
      <LifeBar hero={hero} />
      <span className="flex min-h-7 flex-wrap items-center gap-1">
        {hero.deck ? <ColourPips colours={hero.deck.colors} /> : null}
        {hero.relics.length > 0 ? (
          <span className="ml-auto flex flex-wrap gap-1" aria-label={`Relics of ${hero.name}`} role="group">
            {hero.relics.map((id, i) => (
              <RelicChip key={`${id}-${i}`} card={relicCard(content, id)} owner={hero.name} />
            ))}
          </span>
        ) : null}
      </span>
    </li>
  )
}

/** The run's header. */
export function RunHeader({ view, content }: { view: DelveView; content: DelveContent }) {
  const run = view.run
  return (
    <header data-slot="run-header" className="grid gap-3 rounded-xl border bg-delve-stone p-4 shadow-e1">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <div className="grid min-w-0 gap-1">
          <p className="text-xs font-semibold tracking-wide text-delve uppercase">{runStage(view)}</p>
          <h1 className="text-2xl font-semibold tracking-tight">{content.theme}</h1>
        </div>
        <div className="ml-auto flex flex-wrap items-center gap-x-4 gap-y-2 text-sm">
          <span
            className="inline-flex items-center gap-1 font-medium"
            aria-label={`Heat ${run.heat}`}
            title="Heat raises the bosses' life and wits"
          >
            <FlameIcon
              aria-hidden="true"
              className={cn('size-5', run.heat > 0 ? 'text-delve' : 'text-muted-foreground')}
            />
            <span aria-hidden="true">Heat {run.heat}</span>
          </span>
          <Hearts left={run.hearts} />
          <span className="inline-flex items-center gap-1">
            <span className="text-muted-foreground">Seed</span>
            <span className="font-mono font-medium">{view.seed}</span>
            <CopyButton text={view.seed} label="Copy seed code" />
          </span>
        </div>
      </div>
      {run.status === 'Lobby' ? null : (
        <ul aria-label="Party" className="grid grid-cols-2 gap-2 lg:grid-cols-4">
          {run.heroes.map((hero, i) => (
            <HeroLine key={`${hero.name}-${i}`} hero={hero} you={i === view.me} content={content} />
          ))}
        </ul>
      )}
    </header>
  )
}
