/**
 * The viewer's own spell asks with its clause marked, as the server marks it from where the engine asks: `?prompt=` of
 * `asked-modes` (the modes as it is cast), `asked-number` (a number as it resolves) and `asked-cards` (a card of the hand
 * chosen for its second sentence). The spell stands on the stack of the fixture.
 */
import type { PendingDecision } from '@/play/store'
import type { AskingView, ObjectView } from '@/protocol/generated'
import type { Fixture } from './fixture'

/** A spell of the scene: its name, type and words, and the clause each question marks. */
interface Spell {
  name: string
  types: ObjectView['types']
  text: string
}

const CHARM: Spell = {
  name: "Seeker's Charm",
  types: ['Sorcery'],
  text: 'Choose one —\n• You gain 3 life.\n• Search your library for a creature card, reveal it, put it into your hand, then shuffle.',
}

const GROWTH: Spell = {
  name: 'Measured Growth',
  types: ['Instant'],
  text: 'Draw a card. Put up to two +1/+1 counters on target creature.',
}

const RECALL: Spell = {
  name: 'Careful Recall',
  types: ['Sorcery'],
  text: 'Draw two cards. Then put a card from your hand on top of your library.',
}

/** The UTF-16 span of `words` in `text`; the whole text when they are not in it. */
function spanOf(text: string, words: string): [number, number] {
  const at = text.indexOf(words)
  return at < 0 ? [0, text.length] : [at, at + words.length]
}

/** The fixture with `spell` as the only object on the stack, the viewer's, and the viewer holding priority. */
function withSpell(fixture: Fixture, spell: Spell): Fixture {
  const { view } = fixture
  const id = Math.max(...view.objects.map((object) => object.id)) + 1
  const object: ObjectView = {
    id,
    owner: view.viewer,
    controller: view.viewer,
    zone: 'Stack',
    name: spell.name,
    mana_cost: [{ Generic: 1 }, { Colored: 'Green' }],
    supertypes: [],
    types: spell.types,
    subtypes: [],
    colors: ['Green'],
    power: null,
    toughness: null,
    abilities: [],
    permanent: null,
  }
  return {
    ...fixture,
    view: {
      ...view,
      objects: [...view.objects.filter((one) => one.zone !== 'Stack'), object],
      stack: { count: 1, cards: [id] },
      turn: { ...view.turn, active: view.viewer, step: 'Main1' },
      combat: { attackers: [], attackers_declared: false, blockers_declared: [] },
      combat_forecast: null,
      priority: { holder: view.viewer, passed: [] },
    },
  }
}

/** The id of the spell on the fixture's stack. */
const spellId = (fixture: Fixture): number => fixture.view.stack.cards?.[0] ?? 0

/** What asks: the spell, with its words and the clause of `words` marked. */
function asking(fixture: Fixture, spell: Spell, words: string, resolving: boolean): AskingView {
  const id = spellId(fixture)
  return {
    stack: id,
    source: id,
    name: spell.name,
    text: spell.text,
    clause: spanOf(spell.text, words),
    ...(resolving ? { resolving: true } : {}),
  }
}

/** The tables of the scene, by `?prompt=` name. */
export const ASKED_TABLES: Record<string, (fixture: Fixture) => Fixture> = {
  'asked-modes': (fixture) => withSpell(fixture, CHARM),
  'asked-number': (fixture) => withSpell(fixture, GROWTH),
  'asked-cards': (fixture) => withSpell(fixture, RECALL),
}

/** The decisions of the scene, by `?prompt=` name. */
export const ASKED_PROMPTS: Record<string, (fixture: Fixture) => PendingDecision> = {
  'asked-modes': (fixture) => ({
    id: 1,
    decision: {
      player: fixture.view.viewer,
      kind: {
        ChooseMode: {
          source: spellId(fixture),
          modes: [
            { index: 0, label: 'You gain 3 life' },
            {
              index: 1,
              label:
                'Search your library for a creature card, reveal it, put it into your hand, then shuffle',
            },
          ],
          min: 1,
          max: 1,
          repeat: false,
          can_cancel: true,
        },
      },
    },
    asking: asking(fixture, CHARM, CHARM.text, false),
  }),
  'asked-number': (fixture) => ({
    id: 1,
    decision: {
      player: fixture.view.viewer,
      kind: { ChooseNumber: { source: spellId(fixture), min: 0, max: 2, text: 'Choose how many counters.' } },
    },
    asking: asking(fixture, GROWTH, 'Put up to two +1/+1 counters on target creature.', true),
  }),
  'asked-cards': (fixture) => ({
    id: 1,
    decision: {
      player: fixture.view.viewer,
      kind: {
        ChooseCards: {
          source: spellId(fixture),
          from: fixture.view.players[fixture.view.viewer]?.hand.cards ?? [],
          min: 1,
          max: 1,
          text: 'Put a card from your hand on top of your library.',
        },
      },
    },
    asking: asking(fixture, RECALL, 'Then put a card from your hand on top of your library.', true),
  }),
}
