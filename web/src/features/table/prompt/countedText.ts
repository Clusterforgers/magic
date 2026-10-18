/** The words of a prompt that asks for an exact number of cards: "2 of 3 chosen" and what to do next. */

/** "1 card", "3 cards". */
export function cards(count: number): string {
  return `${count} ${count === 1 ? 'card' : 'cards'}`
}

/**
 * The sentence for `need` cards to `what` ("discard") when `chosen` are picked: the first asks for all of them, then
 * the count so far and what is left to do.
 */
export function countedText(need: number, chosen: number, what: string, intro = ''): string {
  if (need === 0) return `${intro}There are no cards to ${what}. Press Confirm to go on.`
  const progress = `${chosen} of ${need} chosen.`
  if (chosen === 0) return `${intro}Choose ${cards(need)} to ${what}. ${progress}`
  if (chosen < need) {
    const left = need - chosen
    return `${progress} Choose ${left} more ${left === 1 ? 'card' : 'cards'} to ${what}.`
  }
  return `${progress} Press Confirm.`
}

/** Said when a card is chosen beyond the number asked for; nothing is dropped to make room. */
export function refusalText(need: number): string {
  return `Only ${cards(need)} can be chosen. Choose a chosen card again to put it back.`
}
