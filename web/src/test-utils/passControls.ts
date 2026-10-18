/** Finds the controls of the Priority prompt in the page; their words depend on the situation, so tests find them by slot. */

/** The primary button of the Priority prompt, which passes priority ("Next: Combat", "Resolve Bolt", "Continue"...). */
export function primaryButton(): HTMLElement {
  const found = document.querySelector<HTMLElement>('[data-slot="primary-action"]')
  if (found === null) throw new Error('the page has no Priority prompt with a primary button')
  return found
}

/** The primary button, or null when no Priority prompt is on the page. */
export function maybePrimaryButton(): HTMLElement | null {
  return document.querySelector<HTMLElement>('[data-slot="primary-action"]')
}
