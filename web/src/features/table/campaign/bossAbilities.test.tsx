// Goal: every boss of the shipped roster shows each of its abilities in plain words from the content the engine plays (its passive, its own Enrage with the heroes' turn it begins on, Escalation, this fight's modifier, its relic, its phases to come with the share of life at which each begins, the scheme in motion and the Omen with when it comes), as a one-line summary on the table's bar and in full on the intro list; an emblem given twice is listed once with its count; a boss's own triggers are named "Boss: ability" in the log; its commander card is listed as "Phase III: card" until its last phase begins.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { DelveContent, ObjectView, PlayerView } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { GameTable } from '../GameTable'
import { buildFixture } from '../dev/fixture'
import { withBoss } from '../dev/devGame'
import { describeEvent } from '../eventText'
import { bossAbilities, bossOwner, summaryOf } from './bossAbilities'
import { BossAbilityList } from './BossAbilities'
import { commanderLabel } from './bossPhase'
import { DelveContentContext } from './useBossAbilities'

const CONTENT_DIR = join(import.meta.dirname, '../../../../../crates/mtg-campaign/content')

interface RawCard {
  id: string
  name: string
  kind: DelveContent['cards'][number]['kind']
  type_line: string
  text: string
  art: string
  role?: string
  price?: number
  turn?: number
  rarity?: DelveContent['cards'][number]['rarity']
}

interface RawBoss {
  id: string
  name: string
  colors: string
  line: string
  life: number
  life_per_hero: number
  rules: string[]
  enrage: string
  relic: string
  phases: Array<[string, number]>
  art: { card: string; oracle_id: string }
}

interface RawTheme {
  name: string
  text: string
  bosses: RawBoss[]
  schemes: string[]
  modifiers: string[]
}

const cards = ['cards.json', 'boss_cards.json'].flatMap(
  (file) => JSON.parse(readFileSync(join(CONTENT_DIR, file), 'utf8')) as RawCard[],
)
const theme = JSON.parse(readFileSync(join(CONTENT_DIR, 'warrens.json'), 'utf8')) as RawTheme

/** The content as `GET /api/delves/content` serves it, built from the same files. */
const CONTENT: DelveContent = {
  theme: theme.name,
  text: theme.text,
  cards: cards.map((card) => ({
    id: card.id,
    name: card.name,
    kind: card.kind,
    type_line: card.type_line,
    text: card.text,
    art: card.art,
    role: card.role ?? '',
    price: card.price ?? 0,
    turn: card.turn ?? 0,
    rarity: card.rarity ?? 'Common',
  })),
  bosses: theme.bosses.map((boss) => ({
    id: boss.id,
    name: boss.name,
    colors: boss.colors,
    line: boss.line,
    card: boss.rules[0] ?? null,
    phases: boss.phases,
    life: [boss.life, boss.life_per_hero],
    art: boss.art,
  })),
  events: [],
}

const card = (id: string): RawCard => {
  const found = cards.find((entry) => entry.id === id)
  if (found === undefined) throw new Error(`no card ${id}`)
  return found
}

/** The always-on cards of a fight against `boss`: its passive, its Enrage, Escalation, a modifier and its relic. */
function standing(boss: RawBoss): string[] {
  return [...boss.rules, boss.enrage, 'escalation', theme.modifiers[0] ?? '', boss.relic]
}

/** A boss seat as the engine shows it: its emblems in the command zone, phases by name, a scheme and the Omen. */
function bossSeat(
  boss: RawBoss,
  extra: string[] = [],
): { player: PlayerView; objects: Record<number, ObjectView> } {
  const ruleIds = [...standing(boss), ...extra]
  const objects: Record<number, ObjectView> = {}
  ruleIds.forEach((id, at) => {
    objects[900 + at] = { id: 900 + at, name: card(id).name, zone: { Command: 3 } } as ObjectView
  })
  const [scheme = '', omen = ''] = theme.schemes
  const player = {
    seat: 3,
    command: { count: ruleIds.length, cards: Object.keys(objects).map(Number) },
    boss: {
      phases: boss.phases.map(([id]) => card(id).name),
      begun: 0,
      scheme: card(scheme).name,
      omen: card(omen).name,
      omen_in: 2,
    },
  } as unknown as PlayerView
  return { player, objects }
}

describe('bossAbilities over the shipped roster', () => {
  it.each(theme.bosses.map((boss) => [boss.name, boss] as const))('%s', (_, boss) => {
    const { player, objects } = bossSeat(boss)
    const abilities = bossAbilities(player, boss.name, objects, CONTENT)
    const expected = [...standing(boss), ...boss.phases.map(([id]) => id)]
    for (const id of expected) {
      const entry = abilities.find((one) => one.name === card(id).name)
      expect(entry, id).toBeDefined()
      expect(entry?.text).toBe(card(id).text)
      expect(entry?.summary).not.toBe('')
    }
    boss.phases.forEach(([id, percent]) => {
      expect(abilities.find((one) => one.name === card(id).name)?.when).toBe(`At ${percent}% of its life`)
    })
    expect(abilities.find((one) => one.kind === 'enrage')?.when).toBe("From the heroes' turn 7")
    expect(abilities.find((one) => one.kind === 'omen')?.when).toBe('In 2 of its turns')
    expect(abilities.map((one) => one.kind)).toEqual([
      ...boss.rules.map(() => 'passive'),
      'enrage',
      'passive',
      'modifier',
      'relic',
      ...boss.phases.map(() => 'next-phase'),
      'scheme',
      'omen',
    ])
    render(<BossAbilityList abilities={abilities} />)
    const list = within(screen.getByRole('list', { name: 'Boss abilities' }))
    for (const id of expected) expect(list.getByText(card(id).name)).toBeInTheDocument()
  })

  it.each([
    ['before its phases', 0, true],
    ['in its second phase', 1, true],
    ['once its last phase began', 2, false],
  ])('names its commander card with the phase it comes in %s', (_, begun, listed) => {
    const [boss] = theme.bosses
    if (boss === undefined) throw new Error('no boss')
    const { player, objects } = bossSeat(boss)
    const view = { ...player.boss, begun, commander: 'Ashcoat of the Shadow Swarm' }
    const seat = { ...player, boss: view } as PlayerView
    const entry = bossAbilities(seat, boss.name, objects, CONTENT).find((one) =>
      one.key.startsWith('commander-'),
    )
    expect(entry?.name).toBe(listed ? 'Phase III: Ashcoat of the Shadow Swarm' : undefined)
    if (listed) expect(entry?.when).toBe('With its last phase')
  })

  it('numbers the commander card by the boss’s own phases, the Last Bulwark aside', () => {
    expect(commanderLabel(['Phase II — A', 'Phase III — B', 'Last Bulwark'], 'Akroma, Angel of Wrath')).toBe(
      'Phase III: Akroma, Angel of Wrath',
    )
  })

  it('lists an emblem given twice once, with its count', () => {
    const [boss] = theme.bosses
    if (boss === undefined) throw new Error('no boss')
    const { player, objects } = bossSeat(boss, ['reinforcements', 'reinforcements'])
    const names = bossAbilities(player, boss.name, objects, CONTENT).map((one) => one.name)
    expect(names).toContain(`${card('reinforcements').name} ×2`)
    expect(names.filter((name) => name.startsWith(card('reinforcements').name))).toHaveLength(1)
  })

  it('summarises a rule by its first sentence, without reminder text', () => {
    expect(summaryOf(card('reserves').text)).toBe(
      'At the beginning of your upkeep, if you have taken two or more turns, create a 2/2 black Zombie creature token.',
    )
    expect(summaryOf('One.\nTwo.')).toBe('One.')
    expect(summaryOf('')).toBe('')
  })
})

describe('the boss abilities at the table', () => {
  it('shows a chip for each ability with its one-line summary, named for the boss', () => {
    const fixture = withBoss(buildFixture({ seats: 4 }), 0)
    const [boss] = theme.bosses
    if (boss === undefined) throw new Error('no boss')
    const phases = boss.phases.map(([id]) => card(id).name)
    const omen = card(theme.schemes[1] ?? '').name
    const view = {
      ...fixture.view,
      players: fixture.view.players.map((player) =>
        player.boss ? { ...player, boss: { phases, begun: 0, omen } } : player,
      ),
    }
    renderTable({
      fixture: { ...fixture, view },
      ui: (
        <DelveContentContext value={CONTENT}>
          <GameTable />
        </DelveContentContext>
      ),
    })
    const bar = within(screen.getByRole('list', { name: /abilities$/ }))
    for (const [id] of boss.phases) {
      expect(bar.getByText(card(id).name)).toBeInTheDocument()
      expect(bar.getByText(summaryOf(card(id).text))).toBeInTheDocument()
    }
    expect(bar.getByText(omen)).toBeInTheDocument()
  })
})

describe('the log names a boss ability', () => {
  it('as "Boss: ability triggers", and any other source as before', () => {
    const emblem = { id: 7, name: 'Escalation', zone: { Command: 3 } } as ObjectView
    const players = [{ seat: 3, boss: { phases: [], begun: 0 } }] as unknown as PlayerView[]
    const names = {
      seat: (seat: number) => (seat === 3 ? 'Skrix, the Rat Queen' : `Seat ${seat}`),
      card: (id: number) => (id === 7 ? 'Escalation' : 'Llanowar Elves'),
      boss: (id: number) =>
        bossOwner(id === 7 ? emblem : undefined, players, (seat) =>
          seat === 3 ? 'Skrix, the Rat Queen' : '',
        ),
    }
    const triggered = (id: number) =>
      ({
        Public: {
          AbilityTriggered: {
            source: { id, counter: 0 },
            ability: 0,
            controller: 3,
            cause: { subjects: [], snapshots: [] },
            snapshot: {
              id,
              counter: 0,
              controller: 3,
              owner: 3,
              zone: { Command: 3 },
              characteristics: { types: 0, colors: 0, abilities: [] },
              counters: [],
            },
          },
        },
      }) as unknown as Parameters<typeof describeEvent>[0]
    expect(describeEvent(triggered(7), names)?.text).toBe('Skrix, the Rat Queen: Escalation triggers')
    expect(describeEvent(triggered(8), names)?.text).toBe('An ability of Llanowar Elves triggers')
  })
})
