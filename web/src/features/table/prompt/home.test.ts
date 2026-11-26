/** Goal: every decision kind of the registry has one home in every layout (column on Stack or on Log, folded, an open rail, a sheet, a phone), a card's question in the panel only while its Stack tab shows, the game's in the dock, a phone's in the bar; the detail stands inline, as a list, in a window or a phone's payment sheet as `docs/ui-design.md` (Detail and windows) says; board choices and the one-press targets are told apart. */
import { describe, expect, it } from 'vitest'
import type { DecisionName } from '../turnText'
import { answersOnPress, askedByCard, decisionHome, detailPlace, isBoardChoice, WINDOW_FROM } from './home'
import type { Home, HomePlan, SideView } from './home'
import { PROMPTS } from './registry'

/** The layouts a home depends on. */
const LAYOUTS: Record<string, { side: SideView; plan: HomePlan }> = {
  stack: { side: { kind: 'column', open: true, tab: 'stack' }, plan: { phone: false } },
  log: { side: { kind: 'column', open: true, tab: 'log' }, plan: { phone: false } },
  folded: { side: { kind: 'rail', open: false, tab: 'stack' }, plan: { phone: false } },
  rail: { side: { kind: 'rail', open: true, tab: 'stack' }, plan: { phone: false } },
  sheet: { side: { kind: 'sheet', open: false, tab: 'stack' }, plan: { phone: false } },
  phone: { side: { kind: 'sheet', open: false, tab: 'stack' }, plan: { phone: true } },
}

const GAME: Record<string, Home> = {
  stack: 'dock',
  log: 'dock',
  folded: 'dock',
  rail: 'dock',
  sheet: 'dock',
  phone: 'bar',
}
const CARD: Record<string, Home> = {
  stack: 'panel',
  log: 'dock',
  folded: 'dock',
  rail: 'panel',
  sheet: 'dock',
  phone: 'bar',
}

/** Each kind of the registry: who asks it and its detail in the panel and the dock (for 3 things to choose among). */
const KINDS: Record<DecisionName, { card: boolean; panel: string; dock: string }> = {
  Priority: { card: false, panel: 'inline', dock: 'inline' },
  Mulligan: { card: false, panel: 'inline', dock: 'inline' },
  BottomCards: { card: false, panel: 'inline', dock: 'inline' },
  Discard: { card: false, panel: 'inline', dock: 'inline' },
  DeclareAttackers: { card: false, panel: 'inline', dock: 'inline' },
  DeclareBlockers: { card: false, panel: 'inline', dock: 'inline' },
  PayMana: { card: true, panel: 'list', dock: 'inline' },
  AssignDamage: { card: true, panel: 'list', dock: 'window' },
  ChooseTargets: { card: true, panel: 'inline', dock: 'inline' },
  ChooseCards: { card: true, panel: 'inline', dock: 'inline' },
  ChooseMode: { card: true, panel: 'list', dock: 'window' },
  ChooseNumber: { card: true, panel: 'inline', dock: 'inline' },
  ChooseName: { card: true, panel: 'list', dock: 'window' },
  DivideAmount: { card: true, panel: 'list', dock: 'window' },
  OrderCards: { card: true, panel: 'list', dock: 'window' },
  Scry: { card: true, panel: 'list', dock: 'window' },
  YesNo: { card: true, panel: 'inline', dock: 'inline' },
  ChooseOption: { card: true, panel: 'inline', dock: 'inline' },
  ChooseCosts: { card: true, panel: 'list', dock: 'window' },
  OrderTriggers: { card: true, panel: 'list', dock: 'window' },
  ChooseReplacement: { card: true, panel: 'list', dock: 'window' },
  Loop: { card: true, panel: 'inline', dock: 'inline' },
  R706ChooseDice: { card: true, panel: 'inline', dock: 'inline' },
  R509BlockPiles: { card: false, panel: 'inline', dock: 'inline' },
}

describe('decision homes', () => {
  it('has a row for every kind of the registry', () => {
    expect(Object.keys(KINDS).sort()).toEqual(Object.keys(PROMPTS).sort())
  })

  const rows = Object.entries(KINDS).flatMap(([kind, row]) =>
    Object.keys(LAYOUTS).map((layout) => [kind, layout, row.card] as const),
  )
  it.each(rows)('%s on %s has one home', (kind, layout, card) => {
    const { side, plan } = LAYOUTS[layout] ?? LAYOUTS['stack']!
    expect(askedByCard(kind)).toBe(card)
    expect(decisionHome(kind, side, plan)).toBe((card ? CARD : GAME)[layout])
  })

  it.each(Object.entries(KINDS))('%s puts its detail where the home says', (kind, row) => {
    expect(detailPlace(kind, 'panel', 3)).toBe(row.panel)
    expect(detailPlace(kind, 'dock', 3)).toBe(row.dock)
  })

  it('outside a table the dock is the home', () => {
    expect(decisionHome('YesNo', null, { phone: false })).toBe('dock')
  })

  it.each([
    ['ChooseOption', 'panel', 4, false, 'inline'],
    ['ChooseOption', 'panel', 5, false, 'list'],
    ['ChooseOption', 'dock', 5, false, 'window'],
    ['ChooseCards', 'panel', 3, true, 'window'],
    ['ChooseTargets', 'panel', WINDOW_FROM + 1, false, 'window'],
    ['ChooseTargets', 'dock', WINDOW_FROM, false, 'inline'],
    ['PayMana', 'bar', 3, false, 'sheet'],
    ['ChooseMode', 'bar', 3, false, 'window'],
    ['YesNo', 'bar', 2, false, 'inline'],
  ] as const)('%s in the %s with %i (off the table: %s) is %s', (kind, home, count, off, place) => {
    expect(detailPlace(kind, home, count, off)).toBe(place)
  })

  it.each([
    ['ChooseTargets', false, true],
    ['ChooseCards', false, true],
    ['ChooseCards', true, false],
    ['PayMana', false, true],
    ['DeclareAttackers', false, true],
    ['DeclareBlockers', false, true],
    ['AssignDamage', false, true],
    ['Priority', false, false],
    ['YesNo', false, false],
    ['ChooseMode', false, false],
  ] as const)('%s (off the table: %s) is a board choice: %s', (kind, off, board) => {
    expect(isBoardChoice(kind, off)).toBe(board)
  })

  it.each([
    [1, 1, true],
    [0, 1, true],
    [1, 2, false],
    [2, 2, false],
    [0, 3, false],
  ])('targets %i to %i answer on the press: %s', (min, max, press) => {
    expect(answersOnPress({ min, max })).toBe(press)
  })
})
