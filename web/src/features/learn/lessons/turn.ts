import type { Lesson } from '../step'

/** Lesson 5: the steps of a turn, why the game skips, a stop, End turn and Full Control. */
export const TURN: Lesson = {
  id: 'turn',
  title: 'Turns and stops',
  text: 'The steps of a turn, why the game passes for you, setting a stop, End turn and Full Control.',
  minutes: 2,
  steps: [
    {
      id: 'steps',
      kind: 'read',
      title: 'The steps of a turn',
      text: 'Every turn runs through the same steps: untap, draw, main, combat, main, end. The lit one is where the game is.',
      lit: ['turn.ladder'],
    },
    {
      id: 'skips',
      kind: 'read',
      title: 'Skipping',
      text: 'The game passes for you wherever you cannot act, and this bar says what it skipped.',
      lit: ['band'],
    },
    {
      id: 'stop',
      kind: 'do',
      title: 'Set a stop',
      text: "A stop makes the game wait for you at a step. Set one at the opponent's end step.",
      presses: ['stops.side.theirs', 'stops.step.End'],
      allow: { kind: 'stop', side: 'Opponent', step: 'End' },
      done: (facts) => facts.stops.some((stop) => stop.side === 'Opponent' && stop.step === 'End'),
    },
    {
      id: 'end-turn',
      kind: 'do',
      title: 'End turn',
      text: '{@pass.end-turn} passes through the rest of your turn in one press.',
      presses: ['pass.end-turn'],
      allow: { kind: 'endTurn' },
      done: (facts) => facts.turn >= 6,
    },
    {
      id: 'bot',
      kind: 'watch',
      title: "The bot's turn",
      text: 'The bot takes its turn.',
      lit: ['opp.board'],
      done: (facts) => facts.turn === 6 && facts.step === 'End' && facts.decision !== null,
    },
    {
      id: 'stopped',
      kind: 'read',
      title: 'Your stop',
      text: 'The game waits here because of your stop: the last moment to cast an instant before your own turn.',
      lit: ['band'],
    },
    {
      id: 'pass',
      kind: 'do',
      title: 'Pass',
      text: 'Nothing to cast this time. Press {@pass.primary}.',
      presses: ['pass.primary'],
      allow: { kind: 'pass' },
      done: (facts) => facts.turn >= 7,
    },
    {
      id: 'full',
      kind: 'read',
      title: 'Full Control',
      text: 'Full Control stops at every step and never passes for you. Leave it off until you miss it.',
      lit: ['fullcontrol'],
    },
  ],
}
