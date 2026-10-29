// Goal: while blocks are being chosen the prompt and the table agree on what is pending and what is known: each block made is a dashed line from the blocker to its attacker and a sentence "Your X blocks Bo's Y" with a way to undo it; the table's forecast, which knows nothing of those blocks, is shown only while none is made (as the outcome "if you do not block", the marks on cards and the seat's life) and gives way to "outcome shown after you confirm"; undoing a block, cancelling, confirming or answering brings it back or takes the lines away; and chosen attacks are muted dashed lines to the seat or planeswalker each one goes at.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, AttackTarget, BlockOptions, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderForecastTable } from '@/test-utils/forecastTable'
import { forecastSentences } from '../combat/forecastText'
import type { ForecastNames } from '../combat/forecastText'

const SEATS = 3

/** A table of seat 0 facing the declare-blockers decision of the dev fixture, with `answer` spied on. */
function block({ forecast = true, seats = SEATS } = {}) {
  const table = renderForecastTable({ seats, prompt: 'block' })
  if (!forecast) table.setView({ combat_forecast: null })
  const answer = vi.spyOn(table.session, 'answer')
  const kind = must(table.store.getState().pending).decision.kind
  const options: BlockOptions = must('DeclareBlockers' in kind ? kind.DeclareBlockers : null)
  const blockers = options.blockers.map((entry) => entry.object)
  const attackers = must(options.blockers[0]).can_block
  const pairs = (): HTMLElement[] => {
    const list = screen.queryByRole('list', { name: 'Your blocks' })
    return list === null ? [] : within(list).getAllByRole('listitem')
  }
  const outcome = (): HTMLElement | null => table.container.querySelector('[data-slot="block-outcome"]')
  const pending = () => table.lines().filter((line) => line.pending)
  const legend = (): string[] =>
    [...table.container.querySelectorAll('[data-slot="combat-key"] li')].map((item) => item.textContent ?? '')
  const make = async (blocker: ObjectId, attacker: ObjectId): Promise<void> => {
    await table.user.click(table.card(blocker))
    await table.user.click(table.card(attacker))
  }
  const name = (id: ObjectId): string => must(table.store.getState().objectsById[id]).name
  // This table has no server to preview with, so after the debounce the prompt falls back to the old words.
  const fellBack = async (): Promise<void> => {
    await waitFor(() => expect(must(outcome())).toHaveTextContent('Outcome shown after you confirm.'))
  }
  return { ...table, answer, blockers, attackers, pairs, outcome, pending, legend, make, name, fellBack }
}

/** The names the sentences use at the dev fixture: seat 0 is the viewer, the others are called by their seat names. */
const SEAT_NAMES = ['Alice', 'Bo', 'Cy']

describe('the declare-blockers prompt before any block is made', () => {
  it('shows the table forecast as what happens if the player does not block, the lethal bottom line first', () => {
    const t = block()
    const heading = must(t.outcome()).querySelector('p')
    expect(heading).toHaveTextContent('If you do not block')
    const items = within(must(t.outcome())).getAllByRole('listitem')
    expect(items.length).toBeGreaterThanOrEqual(5)
    const mine = must(t.forecast().players.find((player) => player.player === 0))
    expect(mine.lethal).toBe(true)
    expect(items[0]).toHaveTextContent(
      `You would take ${mine.damage} damage, from ${mine.life} to ${mine.life_after} life, which is lethal.`,
    )
    expect(must(t.outcome())).not.toHaveTextContent('Outcome shown after you confirm')
  })

  it('draws the forecast on the table as well: the life of the attacked seat, damage badges, no lines pending', () => {
    const t = block()
    expect(t.life(0)).toHaveAttribute('data-lethal', 'true')
    expect(t.damage().size).toBeGreaterThan(0)
    expect(t.pending()).toEqual([])
    expect(t.legend()).toContain('Attacker dies')
  })

  it('shows no outcome at all when the table carries no forecast', () => {
    const t = block({ forecast: false })
    expect(t.outcome()).toBeNull()
    expect(t.life(0)).toBeNull()
  })

  it('lists the attackers in the same words the table uses for them', () => {
    const t = block()
    const names: ForecastNames = {
      creature: (id) => {
        const object = must(t.store.getState().objectsById[id])
        return `${object.controller === 0 ? 'your' : `${SEAT_NAMES[object.controller]}'s`} ${object.name}`
      },
      player: (seat) => (seat === 0 ? 'you' : (SEAT_NAMES[seat] ?? '?')),
      target: (target: AttackTarget) =>
        'Player' in target ? (target.Player === 0 ? 'you' : (SEAT_NAMES[target.Player] ?? '?')) : '?',
    }
    const sentences = forecastSentences(t.forecast(), names)
    const items = within(must(t.outcome()))
      .getAllByRole('listitem')
      .map((item) => item.textContent)
    // Same sentences, as long as no two creatures of a seat share a name (the table numbers those).
    expect(items).toEqual(sentences)
  })
})

describe('a block being made', () => {
  it('is a sentence, a dashed line to the attacker and a note on both cards, and the forecast gives way', async () => {
    const t = block()
    const [blocker] = t.blockers as [ObjectId]
    const [attacker] = t.attackers as [ObjectId]
    const before = t.damage().size
    expect(before).toBeGreaterThan(0)
    await t.make(blocker, attacker)

    expect(t.pending()).toEqual([
      expect.objectContaining({
        key: `pending-block:${blocker}`,
        from: `card:${blocker}`,
        to: `card:${attacker}`,
        kind: 'block',
        outcome: null,
      }),
    ])
    await t.fellBack()
    expect(within(must(t.outcome())).queryAllByRole('listitem')).toHaveLength(0)
    // What the table knew was about combat without this block, so none of it is on screen now.
    expect(t.life(0)).toBeNull()
    expect(t.damage().size).toBe(0)
    expect(t.doomed()).toEqual([])
    expect(
      t
        .lines()
        .filter((line) => !line.pending)
        .every((line) => line.outcome === null),
    ).toBe(true)
    expect(t.legend()).toContain('Chosen, not confirmed yet')
    expect(screen.queryByRole('list', { name: 'Combat forecast' })).toBeNull()
  })

  it('is taken back with its Remove button, and the forecast returns with the line gone', async () => {
    const t = block()
    const [blocker] = t.blockers as [ObjectId]
    const [attacker] = t.attackers as [ObjectId]
    const forecasted = t.damage()
    await t.make(blocker, attacker)
    await t.user.click(t.card(blocker))
    expect(t.pending()).toEqual([])
    expect(t.damage()).toEqual(forecasted)
    expect(t.life(0)).not.toBeNull()
    expect(must(t.outcome())).toHaveTextContent('If you do not block')
    expect(t.legend()).not.toContain('Chosen, not confirmed yet')
  })

  it('can be undone one at a time: two blockers on one attacker are two lines, then one', async () => {
    const t = block()
    const [first, second] = t.blockers as [ObjectId, ObjectId]
    const [attacker] = t.attackers as [ObjectId]
    await t.make(first, attacker)
    await t.make(second, attacker)
    expect(t.pending().map((line) => [line.key, line.to])).toEqual([
      [`pending-block:${first}`, `card:${attacker}`],
      [`pending-block:${second}`, `card:${attacker}`],
    ])
    await t.user.click(t.card(first))
    expect(t.pending().map((line) => line.key)).toEqual([`pending-block:${second}`])
    await t.fellBack()
    await t.user.click(t.card(second))
    expect(t.pending()).toEqual([])
    expect(must(t.outcome())).toHaveTextContent('If you do not block')
  })

  it('moves its line when the block is changed to another attacker', async () => {
    const t = block()
    const [blocker] = t.blockers as [ObjectId]
    const [first, second] = t.attackers as [ObjectId, ObjectId]
    await t.make(blocker, first)
    const before = must(t.pending()[0])
    expect(before.to).toBe(`card:${first}`)
    // A click on the blocker takes the block back, a second one chooses it again, and the attacker is clicked.
    await t.user.click(t.card(blocker))
    await t.user.click(t.card(blocker))
    await t.user.click(t.card(second))
    const after = must(t.pending()[0])
    expect(after.key).toBe(before.key)
    expect(after.to).toBe(`card:${second}`)
    expect(after.d).not.toBe(before.d)
    expect(t.pending()).toHaveLength(1)
  })

  it('draws no line for a blocker whose attacker is not chosen yet, or when the choice is cancelled', async () => {
    const t = block()
    const [blocker] = t.blockers as [ObjectId]
    await t.user.click(t.card(blocker))
    expect(t.pending()).toEqual([])
    expect(must(t.outcome())).toHaveTextContent('If you do not block')
    await t.user.keyboard('{Escape}')
    expect(t.pending()).toEqual([])
  })

  it('is answered as the pairs made, and the lines go with the prompt; the forecast is the table again', async () => {
    const t = block()
    const [first, second] = t.blockers as [ObjectId, ObjectId]
    const [attacker, other] = t.attackers as [ObjectId, ObjectId]
    await t.make(first, attacker)
    await t.make(second, other)
    expect(t.pending()).toHaveLength(2)
    await t.user.click(screen.getByRole('button', { name: 'Confirm blocks' }))
    expect(t.answer.mock.calls.map(([given]) => given)).toEqual<Answer[]>([
      {
        DeclareBlockers: [
          [first, attacker],
          [second, other],
        ],
      },
    ])
    expect(t.store.getState().pending).toBeNull()
    expect(t.pending()).toEqual([])
    expect(t.container.querySelector('[data-slot="block-outcome"]')).toBeNull()
    expect(t.life(0)).not.toBeNull()
    expect(t.damage().size).toBeGreaterThan(0)
  })

  it('goes with No blocks too, even after a block was made', async () => {
    const t = block()
    const [blocker] = t.blockers as [ObjectId]
    const [attacker] = t.attackers as [ObjectId]
    await t.make(blocker, attacker)
    await t.user.click(screen.getByRole('button', { name: 'No blocks' }))
    expect(t.answer.mock.calls.map(([given]) => given)).toEqual<Answer[]>([{ DeclareBlockers: [] }])
    expect(t.pending()).toEqual([])
  })

  it('says the outcome comes after confirming, even with no forecast on the table', async () => {
    const t = block({ forecast: false })
    const [blocker] = t.blockers as [ObjectId]
    const [attacker] = t.attackers as [ObjectId]
    await t.make(blocker, attacker)
    await t.fellBack()
    await t.user.click(t.card(blocker))
    expect(t.outcome()).toBeNull()
  })

  it('leaves the declared blocks of another defender where they are, plain, while one is chosen', async () => {
    const t = block()
    const declared = t.lines().filter((line) => !line.pending && line.kind === 'block')
    expect(declared.length).toBe(2)
    expect(declared.every((line) => line.outcome !== null)).toBe(true)
    const [blocker] = t.blockers as [ObjectId]
    const [attacker] = t.attackers as [ObjectId]
    await t.make(blocker, attacker)
    expect(
      t
        .lines()
        .filter((line) => !line.pending && line.kind === 'block')
        .map((line) => line.key),
    ).toEqual(declared.map((line) => line.key))
    expect(
      t
        .lines()
        .filter((line) => !line.pending)
        .every((line) => line.outcome === null),
    ).toBe(true)
  })

  it('stays out of the way of a table without lines: nothing is drawn for a prompt with no combat to show', () => {
    const t = block()
    t.setView({ combat: { attackers: [], attackers_declared: false, blockers_declared: [] } })
    expect(t.overlay()).toBeNull()
  })
})

/** A table of seat 0 facing the declare-attackers decision of the dev fixture, with `answer` spied on. */
function attack(seats = 4) {
  const table = renderForecastTable({ seats, prompt: 'attack' })
  const answer = vi.spyOn(table.session, 'answer')
  const kind = must(table.store.getState().pending).decision.kind
  const options = must('DeclareAttackers' in kind ? kind.DeclareAttackers : null)
  const attackers = options.attackers.map((entry) => entry.object)
  const defenders = must(options.attackers[0]).defenders
  const walker = defenders.flatMap((target) => ('Planeswalker' in target ? [target.Planeswalker] : []))[0]
  const pending = () => table.lines().filter((line) => line.pending)
  const legend = (): string[] =>
    [...table.container.querySelectorAll('[data-slot="combat-key"] li')].map((item) => item.textContent ?? '')
  const name = (id: ObjectId): string => must(table.store.getState().objectsById[id]).name
  return { ...table, answer, attackers, defenders, walker: must(walker), pending, legend, name }
}

describe('attacks being chosen', () => {
  it('draws nothing before an attacker is chosen: no combat is declared yet', () => {
    const t = attack()
    expect(t.overlay()).toBeNull()
    expect(t.pending()).toEqual([])
  })

  it('draws a muted dashed line from each chosen attacker to the seat it goes at, and drops it when the attacker is not chosen', async () => {
    const t = attack()
    const [a, b] = t.attackers as [ObjectId, ObjectId]
    await t.user.click(t.card(a))
    await t.user.click(t.card(b))
    expect(t.pending().map((line) => [line.key, line.from, line.to, line.kind, line.outcome])).toEqual([
      [`pending-attack:${a}`, `card:${a}`, 'seat:1', 'attack', null],
      [`pending-attack:${b}`, `card:${b}`, 'seat:1', 'attack', null],
    ])
    expect(t.legend()).toEqual(['Chosen, not confirmed yet'])
    await t.user.click(t.card(a))
    expect(t.pending().map((line) => line.key)).toEqual([`pending-attack:${b}`])
  })

  it('goes at the seat chosen in "Attack at", or at the planeswalker', async () => {
    const t = attack()
    const [a, b, c] = t.attackers as [ObjectId, ObjectId, ObjectId]
    const aim = screen.getByRole('combobox', { name: 'Attack at' })
    await t.user.selectOptions(aim, 'player:3')
    await t.user.click(t.card(a))
    await t.user.selectOptions(aim, `walker:${t.walker}`)
    await t.user.click(t.card(b))
    await t.user.selectOptions(aim, 'player:2')
    await t.user.click(t.card(c))
    expect(t.pending().map((line) => [line.from, line.to])).toEqual([
      [`card:${a}`, 'seat:3'],
      [`card:${b}`, `card:${t.walker}`],
      [`card:${c}`, 'seat:2'],
    ])
  })

  it('follows an attacker that is aimed somewhere else with "All at"', async () => {
    const t = attack()
    const [a] = t.attackers as [ObjectId]
    await t.user.click(t.card(a))
    const before = must(t.pending()[0])
    await t.user.selectOptions(screen.getByRole('combobox', { name: 'Attack at' }), 'player:3')
    await t.user.click(screen.getByRole('button', { name: 'All at Dee' }))
    const after = must(t.pending()[0])
    expect(after.key).toBe(before.key)
    expect(after.to).toBe('seat:3')
    expect(after.d).not.toBe(before.d)
  })

  it('draws every attacker with "Attack with all", and none after Clear', async () => {
    const t = attack()
    await t.user.click(screen.getByRole('button', { name: 'Attack with all' }))
    expect(t.pending().map((line) => line.key)).toEqual(t.attackers.map((id) => `pending-attack:${id}`))
    await t.user.click(screen.getByRole('button', { name: 'Clear' }))
    expect(t.pending()).toEqual([])
    expect(t.overlay()).toBeNull()
  })

  it('goes with the prompt once the attack is sent, and the answer is the pairs chosen', async () => {
    const t = attack()
    const [a, b] = t.attackers as [ObjectId, ObjectId]
    await t.user.click(t.card(a))
    await t.user.click(t.card(b))
    await t.user.click(screen.getByRole('button', { name: 'Confirm attack' }))
    expect(t.answer.mock.calls.map(([given]) => given)).toEqual<Answer[]>([
      {
        DeclareAttackers: [
          [a, { Player: 1 }],
          [b, { Player: 1 }],
        ],
      },
    ])
    expect(t.pending()).toEqual([])
    expect(t.overlay()).toBeNull()
  })

  it('goes with No attack, whatever was chosen', async () => {
    const t = attack()
    await t.user.click(t.card(must(t.attackers[0])))
    expect(t.pending()).toHaveLength(1)
    await t.user.click(screen.getByRole('button', { name: 'No attack' }))
    expect(t.pending()).toEqual([])
  })

  it('touches no forecast: the table has none before attackers are declared', async () => {
    const t = attack()
    await t.user.click(t.card(must(t.attackers[0])))
    expect(t.doomed()).toEqual([])
    expect(t.life(1)).toBeNull()
    expect(screen.queryByRole('list', { name: 'Combat forecast' })).toBeNull()
  })
})
