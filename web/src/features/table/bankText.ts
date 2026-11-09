/** The timer bank in words: how many clock extensions a seat holds, and that one was just spent. */

/** The name of a seat's bank mark; `spent` is true for a few seconds after an extension was used. */
export function bankText(bank: number, spent: boolean): string {
  const held = bank === 1 ? '1 clock extension left' : `${bank} clock extensions left`
  return spent ? `Clock extension used, ${held}` : held
}
