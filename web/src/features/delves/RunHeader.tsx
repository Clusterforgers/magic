/**
 * The head of a run page: the theme, where the run is, Heat, the floor, the timers, the party's hearts, the seed code to copy, and each hero
 * with a life bar, gold and a row of relics whose rules show on hover or focus (edged by rarity), the Satchel, upgrades and banes; a phone folds it to one line.
 */
import {
  BackpackIcon,
  ChevronDownIcon,
  CrownIcon,
  ClockIcon,
  CoinsIcon,
  FlameIcon,
  GemIcon,
  GhostIcon,
  HeartIcon,
  LayersIcon,
  SkullIcon,
  Trash2Icon,
  TriangleAlertIcon,
} from 'lucide-react'
import { useState } from 'react'
import { CopyButton } from '@/components/CopyButton'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'
import type { CustomCardView, DelveContent, DelveView, Hero } from '@/protocol/generated'
import { ColourPips } from './CommanderArt'
import {
  customCard,
  HEARTS,
  heatTier,
  heatWords,
  lifeShare,
  lifeTone,
  MAX_LIFE,
  maxLifeOf,
  runStage,
} from './model'
import { RARITY_TONE } from './rarity'
import { timersText } from './words'
import { isBane, satchelWords, upgradeWords } from './upgrades'

const BAR = { low: 'bg-destructive', hurt: 'bg-warning', fine: 'bg-success' } as const

/** A hero's life as a bar with the number, and the life cap beside it once an event moved it. */
export function LifeBar({ hero, className }: { hero: Hero; className?: string }) {
  const max = maxLifeOf(hero)
  const moved = max !== MAX_LIFE
  const tone = lifeTone(hero.life, max)
  return (
    <span
      role="img"
      aria-label={`${hero.name}: ${hero.life} life${moved ? ` of a ${max} cap` : ''}`}
      className={cn('flex min-w-0 items-center gap-2', className)}
    >
      <span className="h-2 min-w-12 flex-1 overflow-hidden rounded-full bg-muted">
        <span
          data-tone={tone}
          className={cn(
            'block h-full rounded-full transition-[width] duration-slow ease-out motion-reduce:transition-none',
            BAR[tone],
          )}
          style={{ width: `${lifeShare(hero.life, max) * 100}%` }}
        />
      </span>
      <span className="min-w-7 text-right text-sm font-semibold tabular-nums">
        {hero.life}
        {moved ? (
          <span
            data-slot="life-cap"
            className={cn('font-medium', max < MAX_LIFE ? 'text-destructive' : 'text-success')}
          >
            /{max}
          </span>
        ) : null}
      </span>
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
            data-rarity={card.rarity}
            aria-label={`${card.name}, ${card.rarity}${risky ? ', risky' : ''}${owner ? `, relic of ${owner}` : ''}: ${card.text}`}
            className={cn(
              'inline-grid size-7 place-items-center rounded-md border-2 bg-surface',
              RARITY_TONE[card.rarity].border,
              risky ? 'text-destructive' : RARITY_TONE[card.rarity].text,
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
        <span>{card.rarity} relic</span>
        <span>{card.text}</span>
        {risky ? <span className="font-semibold">Risky: it helps the boss too.</span> : null}
      </TooltipContent>
    </Tooltip>
  )
}

/** A hero's Satchel (the cards they earned, cast from it in every fight) and commander upgrades, in words. */
function HeroGear({ hero }: { hero: Hero }) {
  const { lost } = hero
  if (hero.satchel.length === 0 && hero.upgrades.length === 0 && lost.length === 0) return null
  return (
    <ul data-slot="hero-gear" className="grid gap-1 text-xs text-muted-foreground">
      {hero.satchel.length > 0 ? (
        <li data-slot="hero-satchel" className="flex items-start gap-1">
          <BackpackIcon aria-hidden="true" className="mt-px size-3.5 shrink-0 text-delve" />
          <span>{satchelWords(hero.satchel)}</span>
        </li>
      ) : null}
      {lost.length > 0 ? (
        <li data-slot="hero-lost" className="flex items-start gap-1">
          <Trash2Icon aria-hidden="true" className="mt-px size-3.5 shrink-0 text-destructive" />
          <span>Given up: {lost.join(', ')}</span>
        </li>
      ) : null}
      {hero.upgrades.map((upgrade, i) => {
        const bane = isBane(upgrade)
        const Icon = bane ? SkullIcon : CrownIcon
        return (
          <li
            key={`${i}-${upgradeWords(upgrade)}`}
            data-slot="hero-upgrade"
            data-bane={bane || undefined}
            className={cn('flex items-start gap-1', bane && 'text-destructive')}
          >
            <Icon
              aria-hidden="true"
              className={cn('mt-px size-3.5 shrink-0', bane ? 'text-destructive' : 'text-delve')}
            />
            <span>
              {bane ? <span className="sr-only">Bane: </span> : null}
              {upgradeWords(upgrade)}
            </span>
          </li>
        )
      })}
    </ul>
  )
}

/** One hero of the header: name, gold, life bar, colors, relics, the Satchel and the commander upgrades. */
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
              <RelicChip key={`${id}-${i}`} card={customCard(content, id)} owner={hero.name} />
            ))}
          </span>
        ) : null}
      </span>
      <HeroGear hero={hero} />
    </li>
  )
}

/**
 * The run's header. On a phone it folds to one line, the theme and the hearts, with a button that opens the rest (Heat,
 * the timers, the seed and the party), so the room's choice is on the first screen.
 */
export function RunHeader({ view, content }: { view: DelveView; content: DelveContent }) {
  const run = view.run
  const [open, setOpen] = useState(false)
  // Folded on a phone: left out under `sm` until opened.
  const folds = open ? undefined : 'max-sm:hidden'
  return (
    <header
      data-slot="run-header"
      data-open={open || undefined}
      className="grid gap-3 rounded-xl border bg-delve-stone p-4 shadow-e1 max-sm:gap-2 max-sm:px-3 max-sm:py-2"
    >
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 max-sm:flex-nowrap max-sm:gap-x-2">
        <div className="grid min-w-0 gap-1 max-sm:flex-1">
          <p className={cn('text-xs font-semibold tracking-wide text-delve uppercase', folds)}>
            {runStage(view)}
          </p>
          <h1 className="truncate text-2xl font-semibold tracking-tight max-sm:text-lg">{view.theme}</h1>
        </div>
        <div className="ml-auto flex flex-wrap items-center gap-x-4 gap-y-2 text-sm max-sm:shrink-0">
          <span
            className={cn('inline-flex items-center gap-1 font-medium', folds)}
            aria-label={heatWords(run.heat)}
            title={`${heatWords(run.heat)}. Each later boss is harder than the one before.`}
          >
            <FlameIcon
              aria-hidden="true"
              className={cn('size-5', run.heat > 0 ? 'text-delve' : 'text-muted-foreground')}
            />
            <span aria-hidden="true">
              Heat {run.heat} · {heatTier(run.heat).name}
            </span>
          </span>
          <span
            data-slot="floor"
            className={cn('inline-flex items-center gap-1 font-medium', folds)}
            title="Each floor below the first makes every boss stronger"
          >
            <LayersIcon aria-hidden="true" className="size-5 text-muted-foreground" />
            <span>Floor {run.floor}</span>
          </span>
          <span
            className={cn('inline-flex items-center gap-1 font-medium', folds)}
            title="How long a player may take in each fight before the table answers for them"
          >
            <ClockIcon aria-hidden="true" className="size-5 text-muted-foreground" />
            <span>Timers: {timersText(run.timers)}</span>
          </span>
          <Hearts left={run.hearts} />
          <span className={cn('inline-flex items-center gap-1', folds)}>
            <span className="text-muted-foreground">Seed</span>
            <span className="font-mono font-medium">{view.seed}</span>
            <CopyButton text={view.seed} label="Copy seed code" />
          </span>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          data-slot="run-header-toggle"
          aria-expanded={open}
          aria-label={open ? 'Hide the run and the party' : 'Show the run and the party'}
          className="shrink-0 sm:hidden"
          onClick={() => setOpen((was) => !was)}
        >
          <ChevronDownIcon
            aria-hidden="true"
            className={cn('size-5 transition-transform', open && 'rotate-180')}
          />
        </Button>
      </div>
      {run.status === 'Lobby' ? null : (
        <ul aria-label="Party" className={cn('grid grid-cols-2 gap-2 lg:grid-cols-4', folds)}>
          {run.heroes.map((hero, i) => (
            <HeroLine key={`${hero.name}-${i}`} hero={hero} you={i === view.me} content={content} />
          ))}
        </ul>
      )}
    </header>
  )
}
