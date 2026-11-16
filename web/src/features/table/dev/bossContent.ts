/** The campaign content the dev table's boss fight reads its rules from, worded as the shipped content is. */
import type { CustomCardView, DelveContent } from '@/protocol/generated'

const card = (
  id: string,
  name: string,
  kind: CustomCardView['kind'],
  text: string,
  turn = 0,
): CustomCardView => ({
  id,
  name,
  kind,
  type_line: kind,
  text,
  art: '',
  role: '',
  price: 0,
  turn,
})

/** The boss's emblems in its command zone at the dev table: its card and Enrage, the rules every boss has, this fight's
 * modifier, its relic and its phase now. */
export const DEV_BOSS_EMBLEMS = [
  'Skrix, the Rat Queen',
  'Enrage — The Swarm Unending',
  'Escalation',
  'Reinforcements',
  'Warpath',
  'Ironhide',
  "Rat King's Crown",
  'The Nest Stirs',
] as const

/** The dev boss's phases, scheme and Omen (`withBoss`). */
export const DEV_BOSS_PHASES = ['The Nest Stirs', 'The Queen Wakes'] as const
export const DEV_BOSS_SCHEME = 'Gnawing Tide'
export const DEV_BOSS_OMEN = 'All Shall Kneel'

/** The content: one card per ability above, with rules as long as the shipped ones. */
export const DEV_BOSS_CONTENT: DelveContent = {
  theme: 'The Warrens',
  text: 'Tunnels under the city.',
  cards: [
    card(
      'skrix',
      'Skrix, the Rat Queen',
      'Boss',
      'Skrix starts with two extra cards and more life for each hero.\nAt the beginning of your upkeep, create a 1/1 black Rat creature token.',
    ),
    card(
      'escalation',
      'Escalation',
      'Boss',
      'At the beginning of your end step, each opponent loses 1 life for every four turns you have taken.',
    ),
    card(
      'swarm_unending',
      'Enrage — The Swarm Unending',
      'Enrage',
      'Once the heroes have had seven turns, at the beginning of your upkeep, create a 1/1 black Rat creature token for each turn they have had beyond six.',
      7,
    ),
    card('ironhide', 'Ironhide', 'Modifier', 'Creatures you control get +0/+2.'),
    card('rat_kings_crown', "Rat King's Crown", 'BossRelic', 'Rat creatures get +1/+1.'),
    card(
      'reinforcements',
      'Reinforcements',
      'Boss',
      'At the beginning of your upkeep, if you have taken two or more turns, draw a card and create a 2/2 black Zombie creature token. (A boss gets Reinforcements against a party of three or more.)',
    ),
    card(
      'warpath',
      'Warpath',
      'Boss',
      "At end of combat on your turn, if it's the first combat phase of the turn and you have taken four or more turns, untap all creatures you control. After this phase, there is an additional combat phase.",
    ),
    card(
      'nest_stirs',
      'The Nest Stirs',
      'Phase',
      'At the beginning of your upkeep, create a 1/1 black Rat creature token.',
    ),
    card(
      'queen_wakes',
      'The Queen Wakes',
      'Phase',
      'Creatures you control get +1/+0.\nAt the beginning of your end step, each opponent loses 1 life.',
    ),
    card(
      'gnawing_tide',
      'Gnawing Tide',
      'Scheme',
      'At the beginning of your upkeep, each opponent mills two cards and loses 1 life.',
    ),
    card(
      'all_shall_kneel',
      'All Shall Kneel',
      'Scheme',
      'Until your next scheme, creatures your opponents control enter tapped.',
    ),
  ],
  bosses: [
    {
      id: 'skrix_nest',
      name: 'Skrix, the Rat Queen',
      colors: 'B',
      line: 'My children are hungry.',
      card: 'skrix',
      phases: [
        ['nest_stirs', 66],
        ['queen_wakes', 33],
      ],
      life: [26, 30],
      art: { card: 'Ashcoat of the Shadow Swarm', oracle_id: '9cc69ea5-42a2-4306-ac6d-cff5adb20bcb' },
    },
  ],
  events: [],
}
