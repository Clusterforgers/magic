/** The campaign content the dev table's boss fight reads its rules from, worded as the shipped content is. */
import type { CustomCardView, DelveContent } from '@/protocol/generated'

const card = (id: string, name: string, kind: CustomCardView['kind'], text: string): CustomCardView => ({
  id,
  name,
  kind,
  type_line: kind,
  text,
  art: '',
  role: '',
  price: 0,
  rarity: 'Common',
})

/** The content: the dev boss's innate, modifier, relic and phases, with rules as long as the shipped ones. */
export const DEV_BOSS_CONTENT: DelveContent = {
  theme: 'The Warrens',
  text: 'Tunnels under the city.',
  cards: [
    card(
      'skrix_brood_iii',
      'Brood III',
      'Innate',
      'At the beginning of your upkeep, create three 1/1 black Rat creature tokens.',
    ),
    card(
      'deep_pockets',
      'Deep Pockets',
      'Encounter',
      "At the beginning of each player's draw step, that player draws an additional card.",
    ),
    card('whetstone', 'Whetstone', 'Relic', 'Creatures you control get +1/+0.'),
    card(
      'skrix_nest_stirs',
      'The Nest Stirs',
      'Phase',
      'When this phase begins, create a Rat for each creature the heroes control. Rats you control have menace.',
    ),
    card(
      'skrix_plague_tide',
      'Plague Tide',
      'Phase',
      'When this phase begins, double the number of Rats you control. Creature tokens you control get +2/+0.',
    ),
  ],
  bosses: [
    {
      id: 'skrix_nest',
      name: 'Skrix, the Rat Queen',
      final_boss: false,
      colors: 'BG',
      line: 'My children are hungry.',
      innate: 'skrix_brood_i',
      round_phases: ['skrix_nest_stirs', 'skrix_plague_tide'],
      life: [40, 15],
      art: { card: 'Ashcoat of the Shadow Swarm', oracle_id: '9cc69ea5-42a2-4306-ac6d-cff5adb20bcb' },
    },
  ],
  events: [],
}
