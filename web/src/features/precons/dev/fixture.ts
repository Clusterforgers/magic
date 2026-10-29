/** The fixed precon list of `/dev/precons`: every check state, long names, two commanders and none, for screenshots. */
import type { PreconListItem, PreconView } from '@/protocol/generated'
import { board, cardLine, checked, issue, preconItem, preconView, reason } from '../fakePrecons'

/** Text that only the dev fixture holds, which `devBuild.test.ts` looks for in the production bundle. */
export const PRECONS_FIXTURE_MARKER = 'dev-precons-fixture'

const commander = (code: string, name: string, over: Partial<PreconListItem>): PreconListItem =>
  preconItem({ code, name, ...over })

/** The list, newest release first and then by name, as the server sorts it. */
export const FIXTURE_PRECONS: readonly PreconListItem[] = [
  checked(
    commander('mh3-eldrazi-unbound', 'Eldrazi Unbound', {
      set: 'M3C',
      release_date: '2024-06-14',
      commanders: ['Ulalek, Fused Atrocity'],
    }),
    { partial: 2, manual: 3 },
  ),
  checked(
    commander('mh3-graveyard-overdrive', 'Graveyard Overdrive', {
      set: 'M3C',
      release_date: '2024-06-14',
      commanders: ['Disa the Restless'],
    }),
    { partial: 1 },
  ),
  checked(
    commander('mh3-tricky-terrain', 'Tricky Terrain', {
      set: 'M3C',
      release_date: '2024-06-14',
      commanders: ['Satya, Aetherflux Genius'],
    }),
  ),
  checked(
    commander('clb-draconic-domination', 'Draconic Domination', {
      set: 'CLB',
      release_date: '2022-06-10',
      commanders: ['Prosper, Tome-Bound', 'Tasha, the Witch Queen'],
    }),
    { manual: 1 },
  ),
  checked(
    commander(
      'c21-silverquill-statement-with-a-very-long-name-that-wraps-onto-lines',
      'Silverquill Statement of Purpose and Other Words That Make This Name Run Long',
      {
        set: 'C21',
        release_date: '2021-04-23',
        commanders: ['Breena, the Demagogue', 'Lisa, the Bloodthirsty With A Long Name'],
      },
    ),
    { partial: 4, manual: 1 },
  ),
  checked(
    commander('c20-arcane-maelstrom', 'Arcane Maelstrom', {
      set: 'C20',
      release_date: '2020-04-17',
      commanders: ['Kalamax, the Stormsire'],
    }),
    { unplayable: 1, manual: 2 },
  ),
  checked(
    commander('c19-faceless-menace', 'Faceless Menace', {
      set: 'C19',
      release_date: '2019-08-23',
      commanders: ['Volrath, the Shapestealer'],
    }),
    { stale: 1 },
  ),
  checked(
    commander('c18-adaptive-enchantment', 'Adaptive Enchantment', {
      set: 'C18',
      release_date: '2018-08-10',
      commanders: ['Estrid, the Masked'],
    }),
    {},
  ),
  checked(
    commander('c18-exquisite-invention', 'Exquisite Invention', {
      set: 'C18',
      release_date: '2018-08-10',
      commanders: ['Saheeli, the Gifted'],
    }),
    { manual: 1 },
    false,
  ),
  checked(
    commander('c17-arcane-wizardry', 'Arcane Wizardry', {
      set: 'C17',
      release_date: '2017-08-25',
      commanders: ['Inalla, Archmage Ritualist'],
    }),
    { partial: 1 },
  ),
  checked(
    commander('c16-entropic-uprising', 'Entropic Uprising', {
      set: 'C16',
      release_date: '2016-11-11',
      commanders: ['Yidris, Maelstrom Wielder'],
      card_count: 99,
    }),
    { manual: 1 },
  ),
  checked(
    preconItem({
      code: 'chl-blue-red-challenger',
      name: 'Izzet Challenger',
      set: 'CHL',
      deck_type: 'Challenger Deck',
      release_date: '2016-10-11',
      commanders: [],
      card_count: 75,
      format: 'freeform',
    }),
    { partial: 2 },
  ),
  checked(
    preconItem({
      code: 'cmd-forged-in-stone',
      name: 'Forged in Stone',
      set: 'CMD',
      deck_type: null,
      release_date: null,
      commanders: ['Kaalia of the Vast'],
    }),
    { manual: 6, partial: 1 },
  ),
]

/** The details of the fixture precons; the ones with a hand-made card list show what the dialog does with them. */
export function fixtureView(code: string): PreconView | null {
  const item = FIXTURE_PRECONS.find((precon) => precon.code === code)
  if (item === undefined) return null
  if (code === 'mh3-eldrazi-unbound') {
    return preconView(item, [
      cardLine('Eldrazi Temple'),
      cardLine('Kozilek, the Great Distortion', 'partial', {
        reasons: [reason('Discard a card with mana value equal to the spell is not implemented', 40, 90)],
      }),
      cardLine('Emrakul, the Promised End', 'partial', {
        reasons: [
          reason('Control of another player’s turn is not implemented', 12, 60),
          reason('Protection from instants is not implemented', 62, 100),
        ],
      }),
      cardLine('Ugin’s Conjurant', 'manual'),
      cardLine('Thought-Knot Seer', 'manual', { reasons: [reason('No rules for this card yet')] }),
      cardLine('Eldrazi Monument', 'manual', {
        reasons: [reason('Sacrifice a creature at upkeep is not implemented')],
      }),
      cardLine('Sol Ring', 'verified'),
      cardLine('Wastes', 'verified', { count: 8 }),
      cardLine('Command Tower', 'verified'),
    ])
  }
  if (code === 'c18-exquisite-invention') {
    return preconView(item, undefined, {
      issues: [
        issue('The engine accepts this deck only with exactly 100 cards: it has 99.', { code: 'deck_size' }),
        issue('Sol Ring appears twice, and only basic lands may.', { code: 'copies', name: 'Sol Ring' }),
      ],
    })
  }
  if (code === 'c20-arcane-maelstrom') {
    return preconView(item, undefined, {
      boards: [
        board('commander', [cardLine('Kalamax, the Stormsire')]),
        board('main', [
          cardLine('Unplayable Card 1', 'unplayable'),
          cardLine('Manual Card 1', 'manual'),
          cardLine('Island', 'verified', { count: 40 }),
        ]),
        board('sideboard', [cardLine('Wish Target', 'manual')]),
      ],
    })
  }
  return preconView(item)
}
