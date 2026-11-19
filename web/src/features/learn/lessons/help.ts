import type { Lesson } from '../step'

/** Lesson 7: the card preview, keywords, the log, the rules lexicon, the Commands tab and the table menu. */
export const HELP: Lesson = {
  id: 'help',
  title: 'Finding help',
  text: 'Reading a card large, keywords, the log, the rules lexicon, the Commands tab and the table menu.',
  minutes: 2,
  steps: [
    {
      id: 'look',
      kind: 'do',
      title: 'Read a card',
      text: '{look} Serra Angel to read it large.',
      presses: ['board:Serra Angel'],
      allow: { kind: 'look' },
      done: (facts) => facts.page.shown.has('preview'),
    },
    {
      id: 'keywords',
      kind: 'read',
      title: 'Keywords',
      text: 'The preview reads the card large, with its keywords such as Flying and what each one does.',
      lit: ['preview'],
    },
    {
      id: 'log',
      kind: 'read',
      title: 'The log',
      text: 'The log lists everything that happened, newest last.',
      lit: ['log'],
    },
    {
      id: 'lexicon',
      kind: 'do',
      title: 'The rules lexicon',
      text: 'The rules lexicon explains every keyword and rules term. Open it from Help.',
      presses: ['help.menu', 'help.lexicon'],
      allow: { kind: 'look' },
      done: (facts) => facts.page.shown.has('lexicon'),
    },
    {
      id: 'close',
      kind: 'do',
      title: 'Search the lexicon',
      text: 'Search by name or by what a rule does. Close it to go on.',
      presses: ['lexicon.close'],
      allow: { kind: 'look' },
      done: (facts) => !facts.page.shown.has('lexicon'),
    },
    {
      id: 'commands',
      kind: 'read',
      title: 'The Commands tab',
      text: 'A few cards are not automatic yet. The Commands tab plays them by hand; "The manual panel" shows how.',
      lit: ['side.tab.commands'],
    },
    {
      id: 'menu',
      kind: 'read',
      title: 'The table menu',
      text: 'Layout, theme, card images, the order of your hand and Concede are in the menu.',
      lit: ['table.menu'],
    },
  ],
}
