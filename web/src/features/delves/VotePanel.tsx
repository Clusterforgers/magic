/**
 * The vote for the next room, at the top of the run page while the party chooses: the two camps as big buttons with
 * who voted for each, the player's own vote marked, every hero's ballot and what the vote still waits for.
 */
import { BotIcon, CheckIcon, FlameIcon, HourglassIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { DelveAction, DelveView } from '@/protocol/generated'
import { ballots, openChoices, torchOf, voteWaiting, votersOf } from './model'
import { ROOM } from './rooms'

/** The vote panel; it shows nothing while the party is not choosing a room. */
export function VotePanel({
  view,
  act,
  busy,
}: {
  view: DelveView
  act: (action: DelveAction) => void
  busy: boolean
}) {
  const run = view.run
  const choices = openChoices(run)
  if (choices.length === 0) return null
  const myVote = view.me === undefined ? undefined : run.votes[view.me]
  const torch = run.heroes[torchOf(run)]?.name
  const nodeLabel = (id: number) => {
    const node = run.map.nodes.find((n) => n.id === id)
    return node ? ROOM[node.kind].label : `Room ${id}`
  }
  return (
    <section
      aria-labelledby="vote-title"
      data-slot="vote"
      className="grid gap-4 rounded-xl border-2 border-delve bg-delve-stone p-4 shadow-e2"
    >
      <div className="grid gap-1">
        <h2 id="vote-title" className="text-lg font-semibold">
          Vote for the next room
        </h2>
        <p className="text-sm text-muted-foreground">
          The room with the most votes wins; a tie goes to the torch bearer
          {torch ? (
            <>
              , <strong className="text-foreground">{torch}</strong>
            </>
          ) : null}
          . Bot allies vote with the players.
        </p>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        {choices.map((id) => {
          const node = run.map.nodes.find((n) => n.id === id)
          if (!node) return null
          const room = ROOM[node.kind]
          const Icon = room.icon
          const mine = myVote === id
          const voters = votersOf(run, id)
          return (
            <button
              key={id}
              type="button"
              data-choice={id}
              aria-pressed={mine}
              aria-label={mine ? `Your vote: ${room.label}` : `Vote for ${room.label}`}
              disabled={busy || view.me === undefined}
              onClick={() => act({ kind: 'vote', node: id })}
              className={cn(
                'grid min-h-28 gap-2 rounded-xl border-2 bg-surface p-4 text-left shadow-e1 transition-[border-color,box-shadow] duration-base',
                'hover:border-delve focus-visible:ring-2 focus-visible:ring-delve focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-70',
                mine
                  ? 'border-delve ring-2 ring-delve ring-offset-2 ring-offset-delve-stone'
                  : 'border-border',
              )}
            >
              <span className="flex items-center gap-3">
                <span className="grid size-11 shrink-0 place-items-center rounded-full border border-delve text-delve">
                  <Icon aria-hidden="true" className="size-6" />
                </span>
                <span className="grid min-w-0 flex-1">
                  <span className="text-lg font-semibold">{room.label}</span>
                  <span className="text-sm text-muted-foreground">{room.blurb}</span>
                </span>
                {mine ? (
                  <span className="inline-flex shrink-0 items-center gap-1 rounded-full bg-delve px-2 py-1 text-xs font-semibold text-on-delve">
                    <CheckIcon aria-hidden="true" className="size-3.5" />
                    Your vote
                  </span>
                ) : null}
              </span>
              <span className="text-sm">
                <strong>{voters.length}</strong> {voters.length === 1 ? 'vote' : 'votes'}
                {voters.length > 0 ? (
                  <span className="text-muted-foreground"> · {voters.join(', ')}</span>
                ) : null}
              </span>
            </button>
          )
        })}
      </div>
      <ul aria-label="Party votes" className="flex flex-wrap gap-2">
        {ballots(run).map((ballot) => (
          <li
            key={ballot.hero}
            data-ballot={ballot.state}
            className={cn(
              'inline-flex items-center gap-2 rounded-full border bg-surface px-3 py-1 text-sm',
              ballot.state === 'pending' && 'border-dashed',
            )}
          >
            {ballot.state === 'voted' ? (
              <CheckIcon aria-hidden="true" className="size-4 text-success" />
            ) : ballot.state === 'follows' ? (
              <BotIcon aria-hidden="true" className="size-4 text-muted-foreground" />
            ) : (
              <HourglassIcon aria-hidden="true" className="size-4 text-muted-foreground" />
            )}
            <span className="font-medium">
              {ballot.name}
              {ballot.hero === view.me ? ' (you)' : ''}
            </span>
            {torchOf(run) === ballot.hero ? (
              <FlameIcon aria-label="torch bearer" className="size-3.5 text-delve" />
            ) : null}
            <span className="text-muted-foreground">
              {ballot.state === 'voted'
                ? nodeLabel(ballot.node)
                : ballot.state === 'follows'
                  ? 'follows the players'
                  : 'still choosing'}
            </span>
          </li>
        ))}
      </ul>
      <p role="status" className="text-sm font-medium">
        {voteWaiting(run, view.me)}
      </p>
    </section>
  )
}
