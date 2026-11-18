/**
 * The choice on a cleared floor: bank the win or descend to the next floor, as a party vote with every hero's ballot
 * and what a descent changes in plain words.
 */
import { BotIcon, CheckIcon, FlameIcon, HourglassIcon, LayersIcon, TrophyIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { DelveAction, DelveView } from '@/protocol/generated'
import { floorBallots, torchOf } from './model'

const BALLOT_WORDS = {
  descend: 'descend',
  bank: 'bank the win',
  pending: 'still choosing',
  follows: 'votes after the players',
} as const

/** The floor vote; nothing while the floor is not cleared. */
export function DescendPanel({
  view,
  act,
  busy,
}: {
  view: DelveView
  act: (action: DelveAction) => void
  busy: boolean
}) {
  const run = view.run
  if (run.status !== 'FloorCleared') return null
  const mine = view.me === undefined ? undefined : run.floor_votes[view.me]
  const torch = torchOf(run)
  const choices = [
    {
      descend: false,
      label: 'Bank the win',
      blurb: `End the run in victory after ${run.floors_cleared} ${run.floors_cleared === 1 ? 'floor' : 'floors'}.`,
      Icon: TrophyIcon,
    },
    {
      descend: true,
      label: `Descend to floor ${run.floor + 1}`,
      blurb: 'Four new bosses, harder than the last.',
      Icon: LayersIcon,
    },
  ]
  return (
    <section
      aria-labelledby="descend-title"
      data-slot="descend"
      className="grid gap-4 rounded-xl border-2 border-relic bg-delve-stone p-4 shadow-e2"
    >
      <div className="grid gap-1">
        <h2 id="descend-title" className="text-lg font-semibold">
          Floor {run.floor} cleared
        </h2>
        <p className="text-sm text-muted-foreground">
          The party votes: most votes win, a tie goes to the torch bearer
          {run.heroes[torch] ? (
            <>
              , <strong className="text-foreground">{run.heroes[torch].name}</strong>
            </>
          ) : null}
          .
        </p>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        {choices.map(({ descend, label, blurb, Icon }) => {
          const chosen = mine === descend
          return (
            <button
              key={label}
              type="button"
              aria-pressed={chosen}
              disabled={busy || view.me === undefined}
              onClick={() => act({ kind: 'floor_vote', descend })}
              className={cn(
                'grid min-h-24 gap-1 rounded-xl border-2 bg-surface p-4 text-left shadow-e1 transition-[border-color,box-shadow] duration-base',
                'hover:border-delve focus-visible:ring-2 focus-visible:ring-delve focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-70',
                chosen
                  ? 'border-delve ring-2 ring-delve ring-offset-2 ring-offset-delve-stone'
                  : 'border-border',
              )}
            >
              <span className="flex items-center gap-2 text-lg font-semibold">
                <Icon aria-hidden="true" className="size-5 text-delve" />
                {label}
                {chosen ? <CheckIcon aria-label="your vote" className="ml-auto size-4 text-success" /> : null}
              </span>
              <span className="text-sm text-muted-foreground">{blurb}</span>
            </button>
          )
        })}
      </div>
      {view.descent.length > 0 ? (
        <section aria-labelledby="descent-title" className="grid gap-1 text-sm">
          <h3 id="descent-title" className="font-semibold">
            What descending changes
          </h3>
          <ul className="grid list-disc gap-1 pl-4 text-muted-foreground">
            {view.descent.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>
      ) : null}
      <ul aria-label="Floor votes" className="flex flex-wrap gap-2">
        {floorBallots(run).map((ballot) => (
          <li
            key={ballot.hero}
            data-ballot={ballot.state}
            className={cn(
              'inline-flex items-center gap-2 rounded-full border bg-surface px-3 py-1 text-sm',
              ballot.state === 'pending' && 'border-dashed',
            )}
          >
            {ballot.state === 'pending' ? (
              <HourglassIcon aria-hidden="true" className="size-4 text-muted-foreground" />
            ) : ballot.state === 'follows' ? (
              <BotIcon aria-hidden="true" className="size-4 text-muted-foreground" />
            ) : (
              <CheckIcon aria-hidden="true" className="size-4 text-success" />
            )}
            <span className="font-medium">
              {ballot.name}
              {ballot.hero === view.me ? ' (you)' : ''}
            </span>
            {torch === ballot.hero ? (
              <FlameIcon aria-label="torch bearer" className="size-3.5 text-delve" />
            ) : null}
            <span className="text-muted-foreground">{BALLOT_WORDS[ballot.state]}</span>
          </li>
        ))}
      </ul>
    </section>
  )
}
