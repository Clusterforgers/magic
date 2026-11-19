import type { Lesson } from '../step'

/** The manual-panel lesson of "The app": the Commands panel and the card menu, on the `mana` lesson's table. */
export const MANUAL: Lesson = {
  id: 'manual',
  title: 'The manual panel',
  text: 'Play a card by hand: the Commands panel, picking a card, and the card menu for one quick action.',
  minutes: 3,
  steps: [
    {
      id: 'why',
      kind: 'read',
      title: 'Playing by hand',
      text: 'Some cards are not automatic yet. For them you act by hand, and everything you do by hand is in the log for all to see.',
    },
    {
      id: 'open',
      kind: 'do',
      title: 'The Commands panel',
      text: 'Open {@side.tab.commands}. It holds every manual command: drawing, life, tokens, counters and the picked card.',
      presses: ['side.tab.commands'],
      allow: { kind: 'look' },
      done: (facts) => facts.page.shown.has('commands'),
    },
    {
      id: 'move',
      kind: 'do',
      title: 'Move a card by hand',
      text: 'Choose Grizzly Bears in your hand, then move it to the battlefield from the panel. Costs are not checked by hand.',
      presses: ['hand:Grizzly Bears', 'commands'],
      allow: { kind: 'manual' },
      done: (facts) => (facts.page.byHand ?? 0) >= 1,
    },
    {
      id: 'menu',
      kind: 'do',
      title: 'The card menu',
      text: 'A shortcut for one card: right-click it or press and hold it on touch. Tap a Forest from its menu.',
      presses: ['board:Forest'],
      allow: { kind: 'manual' },
      done: (facts) => (facts.page.byHand ?? 0) >= 2,
    },
    {
      id: 'done',
      kind: 'read',
      title: 'That is all',
      text: 'The log shows each action marked "by hand". A card marked By hand tells you which of its rules you play yourself.',
      lit: ['log'],
    },
  ],
}
