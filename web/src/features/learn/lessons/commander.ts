import { has } from '../facts'
import type { Lesson } from '../step'

const GIADA = 'Giada, Font of Hope'

/** Lesson 6: the format, the command zone, casting and returning a commander, the tax and commander damage. */
export const COMMANDER: Lesson = {
  id: 'commander',
  title: 'Commander',
  text: 'Your commander: casting it from the command zone, sending it back there, the tax and commander damage.',
  minutes: 3,
  steps: [
    {
      id: 'format',
      kind: 'read',
      title: 'Commander',
      text: 'Commander: a deck of 100 different cards, 40 life, and one legendary creature that leads the deck.',
      lit: ['own.life'],
    },
    {
      id: 'zone',
      kind: 'read',
      title: 'The command zone',
      text: 'Your commander waits in the command zone, shown beside your hand. You cast it from there like a card in hand.',
      lit: [`command:${GIADA}`],
    },
    {
      id: 'cast',
      kind: 'do',
      title: 'Cast your commander',
      text: 'Choose Giada to cast it.',
      presses: [`command:${GIADA}`],
      allow: { kind: 'cast', name: GIADA },
      done: (facts) => facts.decision === 'PayMana' || has(facts.creatures, GIADA),
    },
    {
      id: 'pay',
      kind: 'do',
      title: 'Pay',
      text: 'Press {@pay.auto}: it picks the lands to tap. Then press {@pay.confirm}.',
      presses: ['pay.auto', 'pay.confirm'],
      allow: { kind: 'pay', how: 'auto' },
      done: (facts) => has(facts.creatures, GIADA) || has(facts.stack, GIADA),
    },
    {
      id: 'pass',
      kind: 'do',
      title: 'End your turn',
      text: 'Press {@pass.primary} to end your turn.',
      presses: ['pass.primary'],
      allow: { kind: 'pass' },
      done: (facts) => facts.turn >= 6,
    },
    {
      id: 'bot',
      kind: 'watch',
      title: 'Removal',
      text: 'The bot casts Doom Blade on your commander.',
      lit: ['stack'],
      done: (facts) => facts.decision === 'YesNo',
    },
    {
      id: 'return',
      kind: 'do',
      title: 'Back to the command zone',
      text: 'Giada would die. A commander may go back to the command zone instead: press {@commander.return}.',
      presses: ['commander.return'],
      allow: { kind: 'choose', yes: true },
      done: (facts) => has(facts.command, GIADA),
    },
    {
      id: 'tax',
      kind: 'read',
      title: 'Commander tax',
      text: 'Each time you cast your commander again it costs {2} more. This is the commander tax.',
      lit: ['command.tax'],
    },
    {
      id: 'recast',
      kind: 'do',
      title: 'Cast it again',
      text: 'Giada now costs {3}{W}. Choose it again.',
      presses: [`command:${GIADA}`],
      allow: { kind: 'cast', name: GIADA },
      done: (facts) => facts.decision === 'PayMana' || has(facts.creatures, GIADA),
    },
    {
      id: 'pay-2',
      kind: 'do',
      title: 'Pay',
      text: 'Press {@pay.auto}: it picks the lands to tap. Then press {@pay.confirm}.',
      presses: ['pay.auto', 'pay.confirm'],
      allow: { kind: 'pay', how: 'auto' },
      done: (facts) => has(facts.creatures, GIADA) || has(facts.stack, GIADA),
    },
    {
      id: 'damage',
      kind: 'read',
      title: 'Commander damage',
      text: 'A player dealt 21 combat damage by one commander loses, whatever their life. Each panel counts it. You are ready to play.',
      lit: ['opp.pod'],
    },
  ],
}
