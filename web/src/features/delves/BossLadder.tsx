/**
 * The ladder as a path down into the dungeon: each boss with its portrait and each camp with its sign, marked cleared,
 * current, next (open to a vote) or locked, and on the next camps who voted for which.
 */
import {
  CheckIcon,
  FlameIcon,
  GemIcon,
  LockIcon,
  MapPinIcon,
  ScrollTextIcon,
  StoreIcon,
  SwordsIcon,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { DelveAction, DelveContent, DelveView, Node, RoomKind, Run } from '@/protocol/generated'
import { BossPortrait } from './CommanderArt'

/** Where a room stands for the party. */
export type NodeState = 'cleared' | 'current' | 'next' | 'locked'

/** The rooms the party votes between now: the next rooms of where it is, while it chooses a path. */
export function openChoices(run: Run): number[] {
  if (run.status !== 'Map' || run.at === null) return []
  return run.map.nodes.find((n) => n.id === run.at)?.next ?? []
}

/** The state of a room: the room the party is in, one it cleared, one it may vote for now, or one still out of reach. */
export function nodeState(node: Node, run: Run): NodeState {
  const over = run.status === 'Victory' || run.status === 'Defeat' || run.status === 'Abandoned'
  if (run.at === node.id && !over && run.status !== 'Map') return 'current'
  if (run.cleared.includes(node.id)) return 'cleared'
  if (openChoices(run).includes(node.id)) return 'next'
  if (run.at === node.id) return 'current'
  return 'locked'
}

/** The heroes who voted for a room, by name. */
export function votersOf(run: Run, node: number): string[] {
  return Object.entries(run.votes)
    .filter(([, v]) => v === node)
    .map(([hero]) => run.heroes[Number(hero)]?.name ?? hero)
}

/** A boss's life for this party and Heat (`docs/delves.md`, Bosses): a fifth more from Heat 1. */
export function bossLife(life: readonly [number, number], heroes: number, heat: number): number {
  const base = life[0] + life[1] * Math.max(0, heroes - 1)
  return heat >= 1 ? Math.round(base * 1.2) : base
}

const ROOM: Record<RoomKind, { label: string; icon: LucideIcon; blurb: string }> = {
  boss: { label: 'Boss', icon: SwordsIcon, blurb: '' },
  treasure: { label: 'Treasure', icon: GemIcon, blurb: 'A relic for each hero' },
  shop: { label: 'Shop', icon: StoreIcon, blurb: 'Cards, relics and a draught' },
  event: { label: 'Event', icon: ScrollTextIcon, blurb: 'A choice with a price' },
  campfire: { label: 'Campfire', icon: FlameIcon, blurb: 'Rest or reforge' },
}

const STATE_WORDS: Record<NodeState, string> = {
  cleared: 'Cleared',
  current: 'You are here',
  next: 'Open to a vote',
  locked: 'Locked',
}

const initials = (name: string): string => name.trim().slice(0, 1).toUpperCase()

/** The mark of a room's state beside its name. */
function StateMark({ state }: { state: NodeState }) {
  const Icon = state === 'cleared' ? CheckIcon : state === 'current' ? MapPinIcon : state === 'locked' ? LockIcon : null
  return (
    <span
      data-state-mark={state}
      className={cn(
        'inline-flex items-center gap-1 text-xs font-medium',
        state === 'cleared' && 'text-success',
        state === 'current' && 'text-delve',
        state === 'next' && 'text-foreground',
        state === 'locked' && 'text-muted-foreground',
      )}
    >
      {Icon ? <Icon aria-hidden="true" className="size-3.5" /> : null}
      {STATE_WORDS[state]}
    </span>
  )
}

/** Who voted for a room: an initial per voter and their names. */
function Voters({ names }: { names: readonly string[] }) {
  if (names.length === 0) return <span className="text-xs text-muted-foreground">No votes yet</span>
  return (
    <span className="flex items-center gap-2">
      <span aria-hidden="true" className="flex -space-x-1">
        {names.map((name) => (
          <span
            key={name}
            className="grid size-6 place-items-center rounded-full border-2 border-surface bg-delve text-xs font-semibold text-on-delve"
          >
            {initials(name)}
          </span>
        ))}
      </span>
      <span className="text-xs">{names.join(', ')}</span>
    </span>
  )
}

function BossStop({
  node,
  state,
  view,
  content,
}: {
  node: Node
  state: NodeState
  view: DelveView
  content: DelveContent
}) {
  const boss = content.bosses.find((b) => b.id === node.boss)
  const name = boss?.name ?? 'A boss'
  const fighting = state === 'current' && typeof view.run.status === 'object'
  const last = node.next.length === 0
  return (
    <li
      data-node={node.id}
      data-state={state}
      data-current={state === 'current' || undefined}
      aria-label={`${last ? 'Final boss' : 'Boss'}: ${name}, ${STATE_WORDS[state]}`}
      className="relative z-10 flex justify-center"
    >
      <div
        className={cn(
          'flex w-full max-w-md items-center gap-3 rounded-xl border-2 bg-surface p-2 pr-4 shadow-e1 transition-[opacity,box-shadow] duration-base',
          state === 'current' && 'border-delve shadow-e2',
          state === 'next' && 'border-delve',
          state === 'cleared' && 'border-success/60',
          state === 'locked' && 'border-dashed opacity-75',
        )}
      >
        <span className="relative shrink-0">
          <BossPortrait
            colours={boss?.colors ?? ''}
            className={cn('size-14 rounded-full border-2 border-surface', state === 'cleared' && 'grayscale')}
          />
          {state === 'cleared' ? (
            <span className="absolute -right-1 -bottom-1 grid size-6 place-items-center rounded-full border-2 border-surface bg-success text-background">
              <CheckIcon aria-hidden="true" className="size-3.5" />
            </span>
          ) : null}
        </span>
        <span className="grid min-w-0 flex-1 gap-0.5">
          <span className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
            {last ? 'Final boss' : 'Boss'}
          </span>
          <span className="truncate font-semibold">{name}</span>
          <span className="flex flex-wrap items-center gap-x-2 text-xs text-muted-foreground">
            {boss ? <span>{bossLife(boss.life, view.run.heroes.length, view.run.heat)} life</span> : null}
            {boss && boss.phases.length > 0 ? <span>{boss.phases.length + 1} phases</span> : null}
          </span>
        </span>
        <span className="shrink-0">
          {fighting ? (
            <span className="inline-flex items-center gap-1 text-xs font-semibold text-delve">
              <SwordsIcon aria-hidden="true" className="size-3.5" />
              Fighting
            </span>
          ) : (
            <StateMark state={state} />
          )}
        </span>
      </div>
    </li>
  )
}

function CampStop({
  node,
  state,
  run,
  mine,
  canVote,
  busy,
  act,
}: {
  node: Node
  state: NodeState
  run: Run
  mine: boolean
  canVote: boolean
  busy: boolean
  act: (action: DelveAction) => void
}) {
  const room = ROOM[node.kind]
  const Icon = room.icon
  const voters = votersOf(run, node.id)
  return (
    <li
      data-node={node.id}
      data-state={state}
      data-current={state === 'current' || undefined}
      aria-label={`${room.label}, ${STATE_WORDS[state]}`}
      className={cn(
        'relative z-10 grid min-w-0 flex-1 basis-40 gap-2 rounded-xl border bg-surface p-3 shadow-e1 transition-[opacity,box-shadow,border-color] duration-base',
        state === 'next' && 'border-2 border-delve',
        mine && 'ring-2 ring-delve ring-offset-2 ring-offset-delve-stone',
        state === 'current' && 'border-2 border-delve shadow-e2',
        state === 'cleared' && 'border-success/60',
        state === 'locked' && 'border-dashed opacity-75',
      )}
    >
      <span className="flex items-center gap-2">
        <span
          className={cn(
            'grid size-9 shrink-0 place-items-center rounded-full border',
            state === 'locked' ? 'text-muted-foreground' : 'border-delve text-delve',
          )}
        >
          <Icon aria-hidden="true" className="size-5" />
        </span>
        <span className="grid min-w-0">
          <span className="font-medium">{room.label}</span>
          <span className="truncate text-xs text-muted-foreground">{room.blurb}</span>
        </span>
      </span>
      {state === 'next' ? (
        <span className="grid gap-2">
          <Voters names={voters} />
          {canVote ? (
            <Button
              size="sm"
              variant={mine ? 'default' : 'outline'}
              aria-pressed={mine}
              disabled={busy}
              onClick={() => act({ kind: 'vote', node: node.id })}
              aria-label={mine ? `Your vote: ${room.label}` : `Vote for ${room.label}`}
            >
              {mine ? 'Your vote' : 'Vote'}
            </Button>
          ) : null}
        </span>
      ) : (
        <StateMark state={state} />
      )}
    </li>
  )
}

/** The ladder: rows from the first boss down to the final one, joined by the path. */
export function BossLadder({
  view,
  content,
  act,
  busy,
}: {
  view: DelveView
  content: DelveContent
  act: (action: DelveAction) => void
  busy: boolean
}) {
  const run = view.run
  const rows: Node[][] = []
  for (const node of run.map.nodes) (rows[node.row] ??= []).push(node)
  const myVote = view.me === undefined ? undefined : run.votes[view.me]
  const torch = run.heroes.length > 0 ? run.cleared.length % run.heroes.length : 0
  const choosing = openChoices(run).length > 0
  const voted = Object.keys(run.votes).length
  return (
    <section aria-labelledby="ladder-title" className="grid gap-3 rounded-xl border bg-delve-stone p-4">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 id="ladder-title" className="text-lg font-semibold">
          The descent
        </h2>
        {choosing ? (
          <p role="status" className="text-sm">
            <strong>
              {voted} of {run.heroes.length}
            </strong>{' '}
            voted · a tie goes to the torch bearer, {run.heroes[torch]?.name}
          </p>
        ) : null}
      </div>
      <ol className="relative grid gap-4 py-2">
        <span
          aria-hidden="true"
          className="absolute inset-y-0 left-1/2 w-1 -translate-x-1/2 rounded-full bg-border-strong/40"
        />
        {rows.map((row, r) => (
          <li key={r} className="relative">
            <ol className={cn('flex flex-wrap justify-center gap-3', row[0]?.kind !== 'boss' && 'gap-x-12')}>
              {row.map((node) => {
                const state = nodeState(node, run)
                return node.kind === 'boss' ? (
                  <BossStop key={node.id} node={node} state={state} view={view} content={content} />
                ) : (
                  <CampStop
                    key={node.id}
                    node={node}
                    state={state}
                    run={run}
                    mine={myVote === node.id}
                    canVote={view.me !== undefined}
                    busy={busy}
                    act={act}
                  />
                )
              })}
            </ol>
          </li>
        ))}
      </ol>
    </section>
  )
}
