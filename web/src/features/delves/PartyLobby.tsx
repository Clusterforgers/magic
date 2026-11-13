/**
 * The lobby of a run: one card per hero with their commander's art, deck, colors and a clear Ready state, the deck
 * picker for the heroes the player may equip, the host's bot allies and Descend.
 */
import { BotIcon, CheckCircle2Icon, HourglassIcon, UserIcon } from 'lucide-react'
import { useState } from 'react'
import { CopyButton } from '@/components/CopyButton'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { cn } from '@/lib/utils'
import type { DeckEntry } from '@/features/game/deckEntries'
import type { Brain, DelveAction, DelveView, Hero } from '@/protocol/generated'
import { ColourPips, CommanderArt } from './CommanderArt'
import { DeckPicker } from './DeckPicker'
import type { LastPlayed } from './deckPicker'

const BRAINS: Brain[] = ['Easy', 'Normal', 'Hard', 'VeryHard']

/** A brain's name in words. */
export const brainText = (brain: Brain): string => (brain === 'VeryHard' ? 'Very hard' : brain)

/** Whether a hero is ready to descend: they bring a deck. */
export const isReady = (hero: Hero): boolean => hero.deck !== null

/** One hero of the lobby. */
export function HeroCard({
  hero,
  you,
  entry,
  onChoose,
  onRemove,
  busy,
}: {
  hero: Hero
  you: boolean
  /** The picker's entry of the hero's deck, for its art; absent for a deck the player's lists do not hold. */
  entry: DeckEntry | undefined
  onChoose?: () => void
  onRemove?: () => void
  busy: boolean
}) {
  const ready = isReady(hero)
  const Icon = hero.bot === null ? UserIcon : BotIcon
  return (
    <li
      data-hero={hero.name}
      data-ready={ready}
      aria-label={`${hero.name}${you ? ' (you)' : ''}: ${ready ? `ready with ${hero.deck?.name ?? ''}` : 'choosing a deck'}`}
      className={cn(
        'grid overflow-hidden rounded-xl border bg-surface shadow-e1',
        ready ? 'border-success/50' : 'border-dashed border-border-strong',
      )}
    >
      <div className="relative">
        <CommanderArt
          cards={entry?.commanders ?? []}
          colours={hero.deck?.colors ?? ''}
          icon={Icon}
          className={cn('aspect-[5/2] w-full', !ready && 'opacity-60 grayscale')}
        />
        <span
          className={cn(
            'absolute top-2 right-2 inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-semibold shadow-e2',
            ready ? 'border border-success bg-raised text-success' : 'border bg-raised text-foreground',
          )}
        >
          {ready ? (
            <CheckCircle2Icon aria-hidden="true" className="size-4" />
          ) : (
            <HourglassIcon aria-hidden="true" className="size-4 motion-safe:animate-pulse" />
          )}
          {ready ? 'Ready' : 'Choosing'}
        </span>
      </div>
      <div className="grid gap-2 p-3">
        <div className="flex items-center gap-2">
          <Icon aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
          <span className="min-w-0 truncate font-medium">{hero.name}</span>
          {you ? <span className="text-xs text-muted-foreground">(you)</span> : null}
          {hero.bot !== null ? (
            <span className="text-xs text-muted-foreground">Bot · {brainText(hero.bot)}</span>
          ) : null}
        </div>
        <div className="flex min-h-6 items-center justify-between gap-2 text-sm">
          {hero.deck ? (
            <>
              <span className="min-w-0 truncate">{hero.deck.name}</span>
              <ColourPips colours={hero.deck.colors} />
            </>
          ) : (
            <span className="text-muted-foreground">No deck yet</span>
          )}
        </div>
        {onChoose || onRemove ? (
          <div className="flex flex-wrap gap-2">
            {onChoose ? (
              <Button
                size="sm"
                variant={ready ? 'outline' : 'default'}
                disabled={busy}
                onClick={onChoose}
                aria-label={`${ready ? 'Change deck' : 'Choose a deck'} for ${hero.name}`}
              >
                {ready ? 'Change deck' : 'Choose a deck'}
              </Button>
            ) : null}
            {onRemove ? (
              <Button size="sm" variant="ghost" disabled={busy} onClick={onRemove}>
                Remove
              </Button>
            ) : null}
          </div>
        ) : null}
      </div>
    </li>
  )
}

/** The lobby: the party's hero cards, the deck picker in a dialog, bot allies and Descend. */
export function PartyLobby({
  view,
  act,
  busy,
  entries,
  played,
  onPlayed,
}: {
  view: DelveView
  act: (action: DelveAction) => void
  busy: boolean
  /** The decks the player may bring; null while they load. */
  entries: readonly DeckEntry[] | null
  played: LastPlayed
  onPlayed: (deck: string) => void
}) {
  const [level, setLevel] = useState<Brain>('Normal')
  const [picking, setPicking] = useState<number | null>(null)
  const heroes = view.run.heroes
  const mayChoose = (hero: Hero, i: number) => i === view.me || (view.host && hero.bot !== null)
  const readyCount = heroes.filter(isReady).length
  const allReady = readyCount === heroes.length
  const pickingHero = picking === null ? undefined : heroes[picking]
  const entryOf = (hero: Hero) => entries?.find((e) => e.id === hero.deck?.id)
  return (
    <section aria-labelledby="party-title" className="grid gap-4">
      <div className="flex flex-wrap items-end justify-between gap-2">
        <div className="grid gap-1">
          <h2 id="party-title" className="text-lg font-semibold">
            Gather the party
          </h2>
          <p className="text-sm text-muted-foreground">
            Up to four heroes, each with a Commander deck. Bot allies fill the gaps.
          </p>
        </div>
        <div className="flex items-center gap-2 text-sm">
          <span className="text-muted-foreground">Invite code</span>
          <strong className="font-mono">{view.code}</strong>
          <CopyButton text={view.code} label="Copy invite code" />
        </div>
      </div>
      <ul className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {heroes.map((hero, i) => (
          <HeroCard
            key={`${hero.name}-${i}`}
            hero={hero}
            you={i === view.me}
            entry={entryOf(hero)}
            busy={busy}
            onChoose={mayChoose(hero, i) ? () => setPicking(i) : undefined}
            onRemove={view.host && i > 0 ? () => act({ kind: 'remove', hero: i }) : undefined}
          />
        ))}
      </ul>
      <div className="surface flex flex-wrap items-center gap-3 p-3">
        <p role="status" className="mr-auto text-sm">
          <strong>
            {readyCount} of {heroes.length}
          </strong>{' '}
          {heroes.length === 1 ? 'hero' : 'heroes'} ready
        </p>
        {view.host ? (
          <>
            <NativeSelect
              aria-label="Bot ally level"
              value={level}
              onChange={(e) => setLevel(e.target.value as Brain)}
            >
              {BRAINS.map((b) => (
                <NativeSelectOption key={b} value={b}>
                  {brainText(b)}
                </NativeSelectOption>
              ))}
            </NativeSelect>
            <Button
              variant="outline"
              disabled={busy || heroes.length >= 4}
              onClick={() => act({ kind: 'add_bot', level })}
            >
              Add a bot ally
            </Button>
            <Button size="lg" disabled={busy || !allReady} onClick={() => act({ kind: 'start' })}>
              Descend
            </Button>
          </>
        ) : (
          <p className="text-sm text-muted-foreground">The host leads the party in once every hero is ready.</p>
        )}
      </div>
      <Dialog open={pickingHero !== undefined} onOpenChange={(open) => !open && setPicking(null)}>
        <DialogContent className="max-w-6xl p-4 sm:p-6">
          <DialogTitle>Choose a deck for {pickingHero?.name}</DialogTitle>
          <DialogDescription>
            A Commander deck for the whole run; won cards replace its basic lands.
          </DialogDescription>
          {pickingHero !== undefined && picking !== null ? (
            <DeckPicker
              heroName={pickingHero.name}
              entries={entries}
              current={pickingHero.deck?.id ?? null}
              played={played}
              onCancel={() => setPicking(null)}
              onTake={(deck) => {
                act({ kind: 'choose_deck', hero: picking, deck })
                onPlayed(deck)
                setPicking(null)
              }}
            />
          ) : null}
        </DialogContent>
      </Dialog>
    </section>
  )
}
