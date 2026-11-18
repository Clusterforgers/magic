/**
 * The intro of a boss fight, shown on the run page while the party is in a fight and in place of the game's lobby
 * before it starts: the boss (portrait, name, line, life), its innate, modifier, phases, Satchel and relics from the
 * plan its game is made with, and the party with the life it carries in. The caller adds the way in.
 */
import type { ReactNode } from 'react'
import { SwordsIcon } from 'lucide-react'
import { levelWords } from '@/features/table/campaign/reworkedBoss'
import type { DelveContent, DelveView } from '@/protocol/generated'
import { BossPortrait, ColourPips } from './CommanderArt'
import { bossLife } from './model'
import { LifeBar } from './RunHeader'
import { ReworkedIntro } from './ReworkedIntro'

/** The boss fight the party is in or about to enter. */
export function BossIntro({
  view,
  content,
  children,
}: {
  view: DelveView
  content: DelveContent
  /** The way in, under the boss's header: a link to the table, a Ready button, or why the fight waits. */
  children?: ReactNode
}) {
  const boss = content.bosses.find((entry) => entry.id === view.boss)
  const fight = view.fight
  const name = fight?.boss ?? boss?.name ?? 'A boss'
  const life = fight?.life ?? (view.boss === undefined ? undefined : bossLife(view, view.boss)) ?? null
  const problems = new Map(view.deck_problems.map((problem) => [problem.hero, problem.reason]))
  return (
    <section
      aria-label="Boss fight"
      className="grid gap-4 rounded-xl border-2 border-delve bg-delve-stone p-4 shadow-e2 motion-safe:animate-in motion-safe:fade-in motion-safe:duration-slow sm:p-6"
    >
      <header className="flex flex-wrap items-center gap-4">
        <BossPortrait
          colours={boss?.colors ?? ''}
          art={boss?.art}
          className="size-20 shrink-0 rounded-full border-2 border-delve"
        />
        <div className="grid min-w-0 flex-1 gap-1">
          <p className="inline-flex items-center gap-1 text-xs font-semibold tracking-wide text-delve uppercase">
            <SwordsIcon aria-hidden="true" className="size-4" />
            Boss fight
          </p>
          <h2 className="text-2xl font-semibold">{name}</h2>
          <p className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
            {boss ? <ColourPips colours={boss.colors} /> : null}
            {life === null ? null : <span>{life} life</span>}
            {fight === undefined ? null : <span>· {levelWords(fight.innate.level)}</span>}
          </p>
          {boss ? <p className="text-sm italic">“{boss.line}”</p> : null}
        </div>
      </header>
      {children}
      <section aria-labelledby="party-title" className="grid gap-2">
        <h3 id="party-title" className="text-sm font-semibold">
          The party
        </h3>
        <p className="text-xs text-muted-foreground">
          Each hero brings the deck chosen for the run and the life carried from the last fight.
        </p>
        <ul className="grid gap-2 sm:grid-cols-2">
          {view.run.heroes.map((hero, at) => (
            <li key={`${at}-${hero.name}`} className="grid gap-1 rounded-lg border bg-surface p-2 text-sm">
              <div className="flex items-center justify-between gap-2">
                <span className="truncate font-medium">{hero.name}</span>
                <span className="truncate text-xs text-muted-foreground">{hero.deck?.name ?? 'No deck'}</span>
              </div>
              <LifeBar hero={hero} />
              {problems.has(at) ? (
                <p role="alert" className="text-xs text-destructive">
                  Deck problem: {problems.get(at)}
                </p>
              ) : null}
            </li>
          ))}
        </ul>
      </section>
      {fight === undefined ? null : <ReworkedIntro fight={fight} name={name} />}
    </section>
  )
}
