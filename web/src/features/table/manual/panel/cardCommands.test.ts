// Goal: the picked card's commands mirror its card menu for each kind of card (permanent, attachment, hand card, graveyard card, an opponent's card, a token, a seat out of the game), each preview says the change in words and sends the exact action, and no manual action the card menu offered before the panel took over its entries is lost: each is in the menu or the panel.
import { describe, expect, it } from 'vitest'
import type { ManualAction, ObjectView } from '@/protocol/generated'
import { card, permanent } from '@/test-utils/cards'
import { manualMenu, OUT_OF_GAME } from '../offers'
import type { Ask, MenuInput } from '../offers'
import { cardCommands, commandsFor, previewOfCard, startCardDraft } from './cardCommands'
import type { CardCommand, CardContext, CardDraft } from './cardCommands'
import { panelGroups, previewOf, startDraft } from './commands'
import type { PanelInput } from './commands'

const SEAT = 0

function input(object: ObjectView, over: Partial<MenuInput> = {}): MenuInput {
  return {
    seat: SEAT,
    object,
    players: [0, 1],
    hasCommandZone: true,
    library: 40,
    counters: [],
    ownerName: 'Bo',
    ...over,
  }
}

const HOST = card({ id: 20, name: 'Bears' })
const EQUIPMENT = permanent({}, { id: 5, name: 'Sword', types: ['Artifact'], subtypes: ['Equipment'] })
const names = ['Ana', 'Bo']

function context(menu: MenuInput): CardContext {
  const objects = { [menu.object.id]: menu.object, [HOST.id]: HOST }
  return { input: menu, objects, nameOf: (seat) => names[seat] ?? `Seat ${seat}` }
}

const ids = (object: ObjectView, over: Partial<MenuInput> = {}): string[] =>
  commandsFor(input(object, over)).map((command) => command.id)

describe('which commands a picked card offers', () => {
  it.each<[string, ObjectView, Partial<MenuInput>, string[]]>([
    ['an untapped creature of the seat', card(), {}, ['move', 'counters', 'tap', 'damage', 'pump']],
    ['a land', card({ types: ['Land'], subtypes: [] }), {}, ['move', 'counters', 'tap', 'damage']],
    [
      'an unattached Aura',
      card({ subtypes: ['Aura'], types: ['Enchantment'] }),
      {},
      ['move', 'counters', 'tap', 'attach', 'damage'],
    ],
    [
      'an attached Equipment',
      { ...EQUIPMENT, attached_to: 20 },
      {},
      ['move', 'counters', 'tap', 'attach', 'unattach', 'damage'],
    ],
    ['a card in the seat hand', card({ zone: { Hand: SEAT }, permanent: null }), {}, ['move', 'discard']],
    [
      'a card in the seat graveyard',
      card({ zone: { Graveyard: SEAT }, permanent: null }),
      {},
      ['move', 'damage'],
    ],
    ['a creature of an opponent', card({ owner: 1, controller: 1 }), {}, ['move', 'counters', 'tap', 'pump']],
    ['a token of the seat', card({ kind: 'Token' }), {}, ['move', 'counters', 'tap', 'damage', 'pump']],
  ])('%s: %j', (_name, object, over, expected) => {
    expect(ids(object, over)).toEqual(expected)
  })

  it('lists the commands of a seat that lost, every one inert with the reason', () => {
    const commands = commandsFor(input(card(), { off: OUT_OF_GAME }))
    expect(commands.length).toBeGreaterThan(3)
    expect(commands.every((command) => command.unavailable === OUT_OF_GAME)).toBe(true)
  })

  it("explains an opponent's card in the move row, and leaves tapping and counters usable", () => {
    const commands = commandsFor(input(card({ owner: 1, controller: 1 })))
    expect(commands[0]?.unavailable).toMatch(/Only Bo/)
    expect(
      commands.filter((command) => command.unavailable === undefined).map((command) => command.id),
    ).toEqual(['counters', 'tap', 'pump'])
  })

  it('names the tap row after what it does', () => {
    expect(commandsFor(input(permanent({ tapped: true }))).find((c) => c.id === 'tap')?.label).toBe('Untap')
    expect(commandsFor(input(permanent({ tapped: false }))).find((c) => c.id === 'tap')?.label).toBe('Tap')
  })

  it('offers the places the menu offers, a place inert with its reason', () => {
    const move = commandsFor(
      input(card({ types: ['Instant'], subtypes: [], zone: { Hand: SEAT }, permanent: null })),
    ).find((c) => c.id === 'move')
    const place = move?.moves?.find((item) => item.key === 'move-Battlefield')
    expect(place?.unavailable).toMatch(/cannot enter/)
    expect(move?.moves?.map((item) => item.key)).toContain('move-Graveyard')
  })
})

/** The preview of the command `id` of `object` with `patch` chosen. */
function previewing(
  object: ObjectView,
  id: CardCommand['id'],
  patch: Partial<CardDraft> = {},
  over: Partial<MenuInput> = {},
) {
  const menu = input(object, over)
  const command = commandsFor(menu).find((c) => c.id === id)
  if (command === undefined) throw new Error(`no command ${id}`)
  return previewOfCard(command, { ...startCardDraft(command, menu), ...patch }, context(menu))
}

describe('what a command previews and sends', () => {
  it.each<[string, ObjectView, CardCommand['id'], Partial<CardDraft>, string, string, ManualAction]>([
    [
      'a move',
      card(),
      'move',
      { dest: 'move-Graveyard' },
      'Grizzly Bears: the battlefield → graveyard',
      'Move to graveyard',
      { Move: { object: 1, to: 'Graveyard' } },
    ],
    [
      'a move from the hand',
      card({ zone: { Hand: SEAT }, permanent: null }),
      'move',
      { dest: 'move-Battlefield' },
      'Grizzly Bears: hand → battlefield',
      'Move to battlefield',
      { Move: { object: 1, to: 'Battlefield' } },
    ],
    [
      'a token removed',
      card({ kind: 'Token' }),
      'move',
      {},
      'Grizzly Bears: the token ceases to exist',
      'Remove the token',
      { Move: { object: 1, to: 'Graveyard' } },
    ],
    [
      'a discard',
      card({ zone: { Hand: SEAT }, permanent: null }),
      'discard',
      {},
      'Grizzly Bears: hand → graveyard, discarded',
      'Discard Grizzly Bears',
      { Discard: { object: 1 } },
    ],
    [
      'a tap',
      card(),
      'tap',
      {},
      'Grizzly Bears: untapped → tapped',
      'Tap Grizzly Bears',
      { Tap: { object: 1 } },
    ],
    [
      'an untap',
      permanent({ tapped: true }),
      'tap',
      {},
      'Grizzly Bears: tapped → untapped',
      'Untap Grizzly Bears',
      { Untap: { object: 1 } },
    ],
    [
      'counters added',
      card(),
      'counters',
      { delta: 2 },
      'Grizzly Bears: +1/+1 0 → 2',
      'Add 2 +1/+1 counters',
      { Counters: { object: 1, counter: '+1/+1', delta: 2 } },
    ],
    [
      'one counter of another kind removed',
      card({ counters: [[{ Other: 'oil' }, 3]] }),
      'counters',
      { kind: 'oil', delta: -1 },
      'Grizzly Bears: oil 3 → 2',
      'Remove 1 oil counter',
      { Counters: { object: 1, counter: { Other: 'oil' }, delta: -1 } },
    ],
    [
      'an attach',
      EQUIPMENT,
      'attach',
      { target: 20 },
      'Sword: attach to Bears',
      'Attach to Bears',
      { Attach: { object: 5, to: 20 } },
    ],
    [
      'an unattach',
      { ...EQUIPMENT, attached_to: 20 },
      'unattach',
      {},
      'Sword: no longer attached to Bears',
      'Unattach',
      { Unattach: { object: 5 } },
    ],
    [
      'damage to a player',
      card(),
      'damage',
      { hitKey: 'player:1', count: 3 },
      'Grizzly Bears deals 3 damage to Bo',
      'Deal 3 damage',
      { Damage: { source: 1, target: { Player: 1 }, amount: 3 } },
    ],
    [
      'a pump',
      card(),
      'pump',
      { power: 2, toughness: -1 },
      'Grizzly Bears: +2/-1 until end of turn',
      'Pump +2/-1',
      { Pump: { object: 1, power: 2, toughness: -1 } },
    ],
  ])('%s', (_name, object, id, patch, preview, apply, action) => {
    expect(previewing(object, id, patch)).toEqual({ preview, apply, action })
  })

  it.each<[string, ObjectView, CardCommand['id'], Partial<CardDraft>, RegExp]>([
    ['counters with no change', card(), 'counters', {}, /Choose an amount/],
    [
      'more counters removed than held',
      card({ counters: [['+1/+1', 1]] }),
      'counters',
      { delta: -2 },
      /Enter a number from 1 to 1/,
    ],
    [
      'a counter of a kind it has none of',
      card(),
      'counters',
      { kind: 'charge', delta: -1 },
      /no charge counter/,
    ],
    [
      'an unnamed counter kind',
      card(),
      'counters',
      { kind: 'other', other: '', delta: 1 },
      /Name the counter/,
    ],
    ['an attach without a target', EQUIPMENT, 'attach', {}, /Choose what to attach it to/],
    ['damage without a target', card(), 'damage', {}, /Choose what is dealt/],
    ['damage of nothing', card(), 'damage', { hitKey: 'player:1', count: 0 }, /Enter an amount/],
    ['a pump of nothing', card(), 'pump', {}, /Change power or toughness/],
    [
      'a move to an inert place',
      card({ types: ['Instant'], subtypes: [], zone: { Hand: SEAT }, permanent: null }),
      'move',
      { dest: 'move-Battlefield' },
      /cannot enter/,
    ],
  ])('says what is missing for %s and sends nothing', (_name, object, id, patch, problem) => {
    const result = previewing(object, id, patch)
    expect('problem' in result && result.problem).toMatch(problem)
    expect('action' in result).toBe(false)
  })

  it('starts a move on the first place that can be used', () => {
    const menu = input(card({ types: ['Instant'], subtypes: [], zone: { Hand: SEAT }, permanent: null }))
    const move = commandsFor(menu).find((c) => c.id === 'move')
    expect(move && startCardDraft(move, menu).dest).toBe('move-Graveyard')
  })
})

describe('no manual action is lost to the shorter menu', () => {
  const ASK_KINDS: Record<Ask, string> = {
    addCounters: 'Counters',
    removeCounters: 'Counters',
    attach: 'Attach',
    token: 'CreateToken',
    gainLife: 'Life',
    loseLife: 'Life',
    addPlayerCounters: 'PlayerCounters',
    removePlayerCounters: 'PlayerCounters',
    draw: 'Draw',
    mill: 'MoveTop',
    moveTop: 'MoveTop',
    damage: 'Damage',
    mana: 'AddMana',
    pump: 'Pump',
  }
  /** Every kind of action the menu offered before the panel's Card, You, Library and Make groups, bar the library actions of the other item. */
  const BEFORE = [
    'Move',
    'MoveTop',
    'Counters',
    'PlayerCounters',
    'Life',
    'CreateToken',
    'Tap',
    'Untap',
    'Draw',
    'Discard',
    'Attach',
    'Unattach',
    'Damage',
    'AddMana',
    'Pump',
  ]
  const kindOf = (action: ManualAction): string => Object.keys(action)[0] ?? ''

  const cards: [string, ObjectView][] = [
    ['a creature', permanent({ tapped: false }, { counters: [['+1/+1', 1]] })],
    ['a tapped creature', permanent({ tapped: true })],
    ['an attached Equipment', { ...EQUIPMENT, attached_to: 20 }],
    ['a hand card', card({ zone: { Hand: SEAT }, permanent: null })],
  ]

  /** What the menu of `object` offers or opens a form for. */
  function menuKinds(object: ObjectView): Set<string> {
    const kinds = new Set<string>()
    for (const group of manualMenu(input(object)).groups) {
      for (const item of group.items) {
        if (item.action !== undefined) kinds.add(kindOf(item.action))
        if (item.ask !== undefined) kinds.add(ASK_KINDS[item.ask])
      }
    }
    return kinds
  }

  /** What the panel's You, Library and Make rows send. */
  function panelKinds(): Set<string> {
    const panel: PanelInput = {
      seat: SEAT,
      name: 'Ana',
      life: 40,
      library: 40,
      hand: 7,
      counters: [],
      off: undefined,
    }
    const kinds = new Set<string>()
    for (const group of panelGroups(panel)) {
      for (const command of group.commands) {
        const draft = {
          ...startDraft(command),
          amount: command.amount?.signed === true ? 1 : (command.amount?.start ?? 0),
        }
        const result = previewOf(command, draft, panel)
        if ('action' in result) kinds.add(kindOf(result.action))
      }
    }
    return kinds
  }

  it('finds every kind of the old menu in the menu, the Card group or the panel', () => {
    const found = new Set<string>([...panelKinds()])
    for (const [, object] of cards) {
      for (const kind of menuKinds(object)) found.add(kind)
      const menu = input(object)
      for (const command of cardCommands(manualMenu(menu))) {
        const draft: Partial<CardDraft> = { delta: 1, target: 20, hitKey: 'player:1', power: 1 }
        const result = previewOfCard(command, { ...startCardDraft(command, menu), ...draft }, context(menu))
        if ('action' in result) found.add(kindOf(result.action))
      }
    }
    expect(BEFORE.filter((kind) => !found.has(kind))).toEqual([])
  })

  it.each(['Life', 'PlayerCounters', 'Draw', 'MoveTop', 'AddMana', 'CreateToken'])(
    'has %s in the panel, which the menu gave up',
    (kind) => {
      expect(panelKinds().has(kind)).toBe(true)
    },
  )

  it.each(cards)('keeps for %s every card action in the Card group that the menu keeps', (_name, object) => {
    const menu = input(object)
    const inMenu = menuKinds(object)
    const group = new Set<string>()
    for (const command of cardCommands(manualMenu(menu))) {
      const draft: Partial<CardDraft> = { delta: 1, target: 20, hitKey: 'player:1', power: 1 }
      const result = previewOfCard(command, { ...startCardDraft(command, menu), ...draft }, context(menu))
      if ('action' in result) group.add(kindOf(result.action))
    }
    for (const kind of [
      'Move',
      'Counters',
      'Tap',
      'Untap',
      'Attach',
      'Unattach',
      'Damage',
      'Pump',
      'Discard',
    ]) {
      if (inMenu.has(kind)) expect(group.has(kind), kind).toBe(true)
    }
  })
})
