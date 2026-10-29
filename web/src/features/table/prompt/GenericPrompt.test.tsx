/** Goal: with no prompt of its own for a decision kind (the registry is replaced by the stand-in for every kind), the table still shows the decision's name and a button for each answer the options allow, the cancel of a cast last, each answer is sent once and echoes the options, a list longer than the buttons says how many it left out, and a kind this version does not know shows its name and cannot crash the table. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, DecisionKind, Target } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { DECISION_LABEL } from '../turnText'
import type { DecisionName } from '../turnText'

vi.mock('./registry', async () => {
  const { UnsupportedPrompt } = await import('./PromptLayout')
  const { DECISION_LABEL: labels } = await import('../turnText')
  return { PROMPTS: Object.fromEntries(Object.keys(labels).map((name) => [name, UnsupportedPrompt])) }
})

const fixture = fixtureOf(2)
const [mine, other] = creaturesOf(fixture, 0) as [number, number]
const foe = must(creaturesOf(fixture, 1)[0])
const spell = must(fixture.view.objects.find((o) => o.zone === 'Stack')).id
const nameOf = (id: number): string => must(fixture.view.objects.find((o) => o.id === id)).name
const object = (id: number): Target => ({ Object: { id, counter: 0 } })
const LINE = 'This decision is not supported in the UI yet. Choose an answer to go on.'

/** A decision of a kind, the labels of the buttons it should get, and the answer of the button at `press`. */
type Row = [name: DecisionName, kind: DecisionKind, labels: string[], press: number, answer: Answer]

const ROWS: Row[] = [
  [
    'Mulligan',
    {
      Mulligan: {
        hand: [mine],
        mulligans_taken: 0,
        can_mulligan: true,
        free_mulligans: 0,
        bottom_if_keep: 0,
        bottom_if_mulligan: 1,
      },
    },
    ['Keep this hand', 'Take a mulligan'],
    1,
    { Mulligan: 'Mulligan' },
  ],
  [
    'BottomCards',
    { BottomCards: { hand: [mine, other, foe], count: 2 } },
    ['Put the first 2 cards on the bottom'],
    0,
    { BottomCards: [mine, other] },
  ],
  [
    'Priority',
    { Priority: { playable_lands: [], castable: [], mana_abilities: [], can_act: false, can_pass: true } },
    ['Pass priority'],
    0,
    { Priority: 'Pass' },
  ],
  [
    'PayMana',
    {
      PayMana: {
        spell: mine,
        cost: [{ Generic: 1 }],
        pool: { counts: [0, 0, 0, 0, 0, 0] },
        sources: [],
        auto_available: true,
        can_cancel: true,
        life: 20,
        max_life: 0,
      },
    },
    ['Pay automatically', 'Cancel the spell'],
    1,
    { PayMana: 'Cancel' },
  ],
  [
    'DeclareAttackers',
    { DeclareAttackers: { attackers: [] } },
    ['Attack with nothing'],
    0,
    { DeclareAttackers: [] },
  ],
  ['DeclareBlockers', { DeclareBlockers: { blockers: [] } }, ['Block nothing'], 0, { DeclareBlockers: [] }],
  [
    'AssignDamage',
    { AssignDamage: { attacker: mine, power: 5, blockers: [foe, other], lethal: [2, 2] } },
    ['Assign lethal damage to each blocker in turn'],
    0,
    {
      AssignDamage: [
        [foe, 2],
        [other, 3],
      ],
    },
  ],
  [
    'Discard',
    { Discard: { hand: [mine, other], count: 1 } },
    ['Discard the first 1 card'],
    0,
    { Discard: [mine] },
  ],
  [
    'ChooseTargets',
    {
      ChooseTargets: {
        source: spell,
        spec: 0,
        legal: [{ Player: 1 }, object(foe)],
        min: 1,
        max: 1,
        can_cancel: true,
      },
    },
    ['Bo', nameOf(foe), 'Cancel the spell'],
    1,
    { ChooseTargets: { Targets: [object(foe)] } },
  ],
  [
    'ChooseMode',
    {
      ChooseMode: {
        source: spell,
        modes: [
          { index: 0, label: 'Draw a card' },
          { index: 1, label: 'Gain 3 life' },
        ],
        min: 1,
        max: 1,
        repeat: false,
        can_cancel: true,
      },
    },
    ['Draw a card', 'Gain 3 life', 'Cancel the spell'],
    2,
    { ChooseMode: 'Cancel' },
  ],
  [
    'ChooseNumber',
    { ChooseNumber: { source: mine, min: 0, max: 3, except: [1], text: 'Choose a value for X.' } },
    ['0', '2', '3'],
    1,
    { ChooseNumber: { Number: 2 } },
  ],
  [
    'DivideAmount',
    { DivideAmount: { source: mine, total: 3, among: [{ Player: 1 }, object(foe)], can_cancel: true } },
    ['Divide evenly', 'Cancel the ability'],
    0,
    {
      DivideAmount: {
        Division: [
          [{ Player: 1 }, 2],
          [object(foe), 1],
        ],
      },
    },
  ],
  [
    'ChooseCards',
    { ChooseCards: { source: foe, from: [mine, other], min: 1, max: 1 } },
    [nameOf(mine), nameOf(other)],
    1,
    { ChooseCards: [other] },
  ],
  [
    'OrderCards',
    { OrderCards: { source: mine, cards: [other, mine] } },
    ['Keep the order'],
    0,
    { OrderCards: [other, mine] },
  ],
  ['YesNo', { YesNo: { source: mine, text: 'Draw a card?' } }, ['Yes', 'No'], 0, { YesNo: true }],
  [
    'ChooseOption',
    { ChooseOption: { source: mine, options: [{ Color: 'Red' }, { Label: 'Fire' }] } },
    ['Red', 'Fire'],
    1,
    { ChooseOption: 1 },
  ],
  [
    'ChooseCosts',
    { ChooseCosts: { source: mine, optional: [{ index: 0, label: 'Kicker {2}{R}' }] } },
    ['Pay no optional cost', 'Kicker {2}{R}'],
    1,
    { ChooseCosts: { Costs: [0] } },
  ],
  [
    'OrderTriggers',
    {
      OrderTriggers: {
        triggers: [
          { source: { id: mine, counter: 0 }, ability: 0, when: 'When it dies' },
          { source: { id: other, counter: 0 }, ability: 0, when: 'When it enters' },
        ],
      },
    },
    ['In the order they triggered'],
    0,
    { OrderTriggers: [0, 1] },
  ],
  [
    'ChooseReplacement',
    {
      ChooseReplacement: {
        candidates: [
          { source: mine, ability: 0 },
          { source: other, ability: 0 },
        ],
      },
    },
    [`${nameOf(mine)} first`, `${nameOf(other)} first`],
    1,
    { ChooseReplacement: 1 },
  ],
]

const buttons = (): string[] =>
  within(screen.getByRole('group', { name: 'Answers' }))
    .getAllByRole('button')
    .map((button) => button.textContent ?? '')

describe('a decision with no prompt of its own', () => {
  it('is covered for every kind', () => {
    expect(new Set(ROWS.map(([name]) => name))).toEqual(new Set(Object.keys(DECISION_LABEL)))
  })

  it.each(ROWS)('%s: names the decision and offers its answers', (name, kind, labels, press, answer) => {
    const table = renderDecision(kind)
    const line = must(table.container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(line).toHaveAttribute('data-decision', name)
    expect(line).toHaveTextContent(`Your decision: ${DECISION_LABEL[name]}`)
    expect(table.text()).toBe(LINE)
    expect(buttons()).toEqual(labels)
    expect(table.answer).not.toHaveBeenCalled()
    return table.user.click(screen.getByRole('button', { name: must(labels[press]) })).then(() => {
      expect(table.answer.mock.calls).toEqual([[answer]])
    })
  })

  it('sends once for a double click', async () => {
    const { user, answer } = renderDecision({ YesNo: { source: mine } })
    await user.dblClick(screen.getByRole('button', { name: 'No' }))
    expect(answer.mock.calls).toEqual([[{ YesNo: false }]])
  })

  it('puts the cancel of a cast last and marks it', () => {
    renderDecision({
      ChooseMode: {
        source: mine,
        modes: [{ index: 0, label: 'Draw a card' }],
        min: 1,
        max: 1,
        repeat: false,
        can_cancel: true,
      },
    })
    const all = within(screen.getByRole('group', { name: 'Answers' })).getAllByRole('button')
    expect(all.map((button) => button.hasAttribute('data-cancel'))).toEqual([false, true])
  })

  it.each<[string, number, string]>([
    ['an ability of a permanent', mine, 'Cancel the ability'],
    ['a spell on the stack', spell, 'Cancel the spell'],
  ])('words the cancel of %s', async (_what, source, words) => {
    const { user, answer } = renderDecision({ ChooseCosts: { source, optional: [], can_cancel: true } })
    await user.click(screen.getByRole('button', { name: words }))
    expect(answer.mock.calls).toEqual([[{ ChooseCosts: 'Cancel' }]])
  })
})

describe('what cannot be answered by a button', () => {
  it.each<[string, DecisionKind]>([
    ['an attack a rule requires', { DeclareAttackers: { attackers: [], required: [mine] } }],
    ['a block a rule requires', { DeclareBlockers: { blockers: [], required: [mine] } }],
    ['a damage split with no blocker', { AssignDamage: { attacker: mine, power: 2, blockers: [] } }],
    [
      'a priority that cannot pass',
      { Priority: { playable_lands: [], castable: [], mana_abilities: [], can_act: true, can_pass: false } },
    ],
    ['a list of options with nothing in it', { ChooseOption: { source: mine, options: [] } }],
  ])('%s says so and offers no button', (_what, kind) => {
    const table = renderDecision(kind)
    expect(table.text()).toBe('This decision is not supported in the UI yet')
    expect(screen.queryByRole('group', { name: 'Answers' })).toBeNull()
  })
})

describe('a long list', () => {
  it('shows the first twelve cards and says how many it left out', () => {
    const from = Array.from({ length: 20 }, (_, at) => 1000 + at)
    const table = renderDecision({ ChooseCards: { source: mine, from, min: 1, max: 1 } })
    expect(buttons()).toHaveLength(12)
    expect(must(table.container.querySelector('[data-slot="generic-unlisted"]')).textContent).toBe(
      '8 more not shown',
    )
  })
})

describe('a decision of a kind this version does not know', () => {
  it('shows its key as the name, says it is not supported and offers nothing', () => {
    const table = renderDecision({ Frobnicate: { source: mine } } as unknown as DecisionKind)
    const line = must(table.container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(line).toHaveAttribute('data-decision', 'Frobnicate')
    expect(line).toHaveTextContent('Your decision: Frobnicate')
    expect(table.text()).toBe('This decision is not supported in the UI yet')
    expect(screen.queryByRole('group', { name: 'Answers' })).toBeNull()
  })
})
