import { Link } from '@tanstack/react-router'
import { ScrollTextIcon } from 'lucide-react'
import { buttonVariants } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { DelveContent, FoughtFight, Run } from '@/protocol/generated'

/**
 * The run's fight log: each fight whose game ended, newest first, with its boss, whether the party won, and a link to
 * the recorded game (its replay, with the whole log of the fight). Nothing while no fight has ended.
 */
export function FightLog({ run, content }: { run: Run; content: DelveContent }) {
  const fights = run.fights
  if (fights.length === 0) return null
  const bossOf = (fight: FoughtFight): string => {
    const id = fight.boss ?? run.map.nodes.find((node) => node.id === fight.room)?.boss ?? null
    const name = content.bosses.find((boss) => boss.id === id)?.name ?? 'The boss'
    return run.floor > 1 ? `${name} (floor ${fight.floor})` : name
  }
  return (
    <section aria-labelledby="fight-log-title" className="surface grid gap-2 p-3 text-sm">
      <h2 id="fight-log-title" className="font-semibold">
        Fight log
      </h2>
      <ol className="grid gap-1">
        {fights
          .map((fight, at) => ({ ...fight, at }))
          .reverse()
          .map((fight) => (
            <li key={fight.game} data-won={fight.won} className="flex min-w-0 items-center gap-2">
              <span className="min-w-0 flex-1 truncate">
                <span className="font-medium">{bossOf(fight)}</span>
                <span className={cn('ml-2', fight.won ? 'text-success' : 'text-muted-foreground')}>
                  {fight.won ? 'Won' : 'Lost'}
                </span>
              </span>
              <Link
                to="/replay/$gameId"
                params={{ gameId: fight.game }}
                aria-label={`Fight log of fight ${fight.at + 1} against ${bossOf(fight)}`}
                className={cn(buttonVariants({ size: 'sm', variant: 'outline' }), 'shrink-0')}
              >
                <ScrollTextIcon aria-hidden="true" />
                Fight log
              </Link>
            </li>
          ))}
      </ol>
    </section>
  )
}
