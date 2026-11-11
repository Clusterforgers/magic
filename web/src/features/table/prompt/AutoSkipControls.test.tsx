/** Goal: the seat's own ability asking yes or no on top of the stack offers "Always Yes" and "Always No", which send the rule keyed by the ability's name and words and then the answer, and no other question offers them; the trigger-order prompt sends "Always use this order" as the confirmed order by source name and ability before the answer, and only when ticked; the Full Control menu lists every rule with a remove button and says Full Control sets them aside. */
import { screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import type { Answer, AutoSkip, DecisionKind, ObjectView, TriggerEntry } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf } from '@/test-utils/combatTable'
import { renderTable } from '@/test-utils/tableHarness'
import { answerKeyOf, orderRule, ruleKey, ruleWords } from '../autoSkips'

const fixture = fixtureOf(2)
const mine = must(creaturesOf(fixture, 0)[0])
const other = must(creaturesOf(fixture, 0)[1])
const theirs = must(creaturesOf(fixture, 1)[0])
const nameOf = (id: number): string => must(fixture.view.objects.find((o) => o.id === id)).name
const ABILITY = 900
const WORDS = 'When a creature dies, you may gain 3 life.'

/** An ability of `controller` from `source` on top of the stack, or a spell when `spell`. */
function onTop(source: number, controller: number, spell = false) {
  return (state: PlayState): PlayState => {
    const base = must(state.objectsById[source])
    const top: ObjectView = spell
      ? { ...base, id: ABILITY, zone: 'Stack', controller }
      : { ...base, id: ABILITY, zone: 'Stack', kind: 'Ability', source, controller, rules_text: WORDS }
    const view = must(state.view)
    return {
      ...state,
      view: {
        ...view,
        objects: [...view.objects, top],
        table: { ...view.table, stack: { count: 1, cards: [ABILITY] } },
      },
      objectsById: { ...state.objectsById, [ABILITY]: top },
    }
  }
}

function mount(kind: DecisionKind, patch: (state: PlayState) => PlayState = (s) => s) {
  const answer = vi.fn((_answer: Answer) => true)
  const autoSkip = vi.fn((_rule: AutoSkip) => true)
  const table = renderTable({
    fixture,
    session: { answer, autoSkip },
    state: (state) => ({ ...patch(state), pending: { id: 7, decision: { player: 0, kind } } }),
  })
  return { ...table, answer, autoSkip }
}

describe('always answer', () => {
  it.each([true, false])('Always %s sends the rule for the ability, then the answer', async (yes) => {
    const { user, answer, autoSkip } = mount({ YesNo: { source: mine, text: 'Gain 3 life?' } }, onTop(mine, 0))
    const word = yes ? 'Yes' : 'No'
    await user.click(screen.getByRole('button', { name: `Always answer ${word} for ${nameOf(mine)}` }))
    expect(autoSkip).toHaveBeenCalledExactlyOnceWith({
      kind: 'Answer',
      ability: { name: nameOf(mine), text: WORDS },
      yes,
    })
    expect(answer).toHaveBeenCalledExactlyOnceWith({ YesNo: yes })
    expect(autoSkip.mock.invocationCallOrder[0]).toBeLessThan(must(answer.mock.invocationCallOrder[0]))
  })

  it('cannot always say Yes to a payment that cannot be made', () => {
    mount({ YesNo: { source: mine, text: 'Pay {1}?', only_no: true } }, onTop(mine, 0))
    expect(screen.getByRole('button', { name: /^Always answer Yes/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: /^Always answer No/ })).toBeEnabled()
  })

  it.each([
    ['an empty stack', (s: PlayState) => s],
    ["an opponent's ability", onTop(theirs, 1)],
    ['an ability of another source', onTop(other, 0)],
    ['a spell', onTop(mine, 0, true)],
  ])('is not offered over %s', (_, patch) => {
    mount({ YesNo: { source: mine, text: 'Gain 3 life?' } }, patch)
    expect(screen.queryByRole('button', { name: /^Always answer/ })).toBeNull()
  })
})

describe('always use this order', () => {
  const entries: TriggerEntry[] = [
    { source: { id: mine, counter: 0 }, ability: 0, when: 'When a creature dies' },
    { source: { id: other, counter: 0 }, ability: 2, when: 'When a creature dies' },
  ]

  it.each([true, false])('ticked %s: Confirm order sends the rule first only when ticked', async (ticked) => {
    const { user, answer, autoSkip } = mount({ OrderTriggers: { triggers: entries } })
    const box = screen.getByRole('checkbox', { name: 'Always use this order' })
    expect(box).not.toBeChecked()
    if (ticked) await user.click(box)
    await user.click(screen.getByRole('button', { name: 'Confirm order' }))
    expect(answer).toHaveBeenCalledExactlyOnceWith({ OrderTriggers: [0, 1] })
    if (ticked) {
      expect(autoSkip).toHaveBeenCalledExactlyOnceWith({
        kind: 'Order',
        triggers: [
          { name: nameOf(mine), ability: 0 },
          { name: nameOf(other), ability: 2 },
        ],
      })
    } else {
      expect(autoSkip).not.toHaveBeenCalled()
    }
  })
})

describe('the rules', () => {
  const answerRule: AutoSkip = { kind: 'Answer', ability: { name: 'Owl', text: WORDS }, yes: true }
  const order: AutoSkip = {
    kind: 'Order',
    triggers: [
      { name: 'First', ability: 0 },
      { name: 'Second', ability: 1 },
    ],
  }

  it.each([
    [answerRule, { ...answerRule, yes: false }, true],
    [order, { kind: 'Order', triggers: [...(order.kind === 'Order' ? order.triggers : [])].reverse() }, true],
    [answerRule, { ...answerRule, ability: { name: 'Owl', text: '' } }, false],
    [order, { kind: 'Order', triggers: [{ name: 'First', ability: 0 }, { name: 'Third', ability: 0 }] }, false],
  ] as [AutoSkip, AutoSkip, boolean][])('%o and %o share a key: %s', (a, b, same) => {
    expect(ruleKey(a) === ruleKey(b)).toBe(same)
  })

  it.each([
    [[0, 1], ['A', 'B'], [{ name: 'A', ability: 0 }, { name: 'B', ability: 2 }]],
    [[1, 0], ['A', 'B'], [{ name: 'B', ability: 2 }, { name: 'A', ability: 0 }]],
    [[0, 1], ['A', ''], null],
    [[0], ['A', 'B'], null],
  ] as const)('order %o of %o is keyed as %o', (picked, names, triggers) => {
    const entries: TriggerEntry[] = [
      { source: { id: 1, counter: 0 }, ability: 0 },
      { source: { id: 2, counter: 0 }, ability: 2 },
    ]
    const rule = orderRule(entries, names, picked)
    expect(rule).toEqual(triggers === null ? null : { kind: 'Order', triggers })
  })

  it('keys an answer only by an ability of the seat from the asking source', () => {
    const ability = { id: 5, kind: 'Ability', name: 'Owl', controller: 0, source: 3, rules_text: WORDS } as ObjectView
    expect(answerKeyOf(ability, 0, 3)).toEqual({ name: 'Owl', text: WORDS })
    expect(answerKeyOf(ability, 1, 3)).toBeNull()
    expect(answerKeyOf(ability, 0, 4)).toBeNull()
    expect(answerKeyOf({ ...ability, name: '' }, 0, 3)).toBeNull()
  })

  const fullControlOptions = (): HTMLElement => screen.getByRole('button', { name: 'Full Control options' })

  it.each(['Off', 'Held'] as const)('in mode %s the menu lists each rule and a click removes it', async (mode) => {
    const cancelAutoSkip = vi.fn((_rule: AutoSkip) => true)
    const { user } = renderTable({
      session: { cancelAutoSkip },
      state: (state) => ({ ...state, autoSkips: [answerRule, order], fullControl: mode }),
    })
    await user.click(fullControlOptions())
    const label = await screen.findByText('Auto-skips')
    expect(label.textContent).toBe(mode === 'Off' ? 'Auto-skips' : 'Auto-skips (off in Full Control)')
    const items = screen.getAllByRole('menuitem', { name: /^Stop (answering|ordering)/ })
    expect(items.map((item) => item.textContent)).toEqual([ruleWords(answerRule).label, ruleWords(order).label])
    expect(items[1]).toHaveAccessibleName('Stop ordering First, then Second')
    await user.click(must(items[0]))
    expect(cancelAutoSkip).toHaveBeenCalledExactlyOnceWith(answerRule)
  })

  it('lists nothing without a rule', async () => {
    const { user } = renderTable({ state: (state) => ({ ...state, autoSkips: [] }) })
    await user.click(fullControlOptions())
    await screen.findByRole('menuitemcheckbox', { name: /End my turn by hand/ })
    expect(screen.queryByText('Auto-skips')).toBeNull()
  })
})
