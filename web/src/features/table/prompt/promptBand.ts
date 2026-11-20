/** Rules of the question's line in a decision's home (`promptBand.css`). */

/** Whether a box's text is cut by its line clamp: what it would take is taller or wider than what it shows. */
export function isClamped(
  box: Pick<HTMLElement, 'scrollHeight' | 'clientHeight' | 'scrollWidth' | 'clientWidth'>,
): boolean {
  return box.scrollHeight > box.clientHeight + 1 || box.scrollWidth > box.clientWidth + 1
}
