/** Marks the one kind of element that may carry a seat token: a link meant to be handed to a player. */
export const SHARE_LINK = '[data-share-link]'

/**
 * Everything on screen that could show a secret, share links excepted: the markup with all its attributes, the
 * text, the value of every form control, and the tab title. A test asserts a token is nowhere in the result.
 */
export function visibleOutsideShareLinks(root: Element): string {
  const copy = root.cloneNode(true)
  if (!(copy instanceof Element)) throw new Error('the test expected an element')
  copy.querySelectorAll(SHARE_LINK).forEach((link) => link.remove())
  const values = [...copy.querySelectorAll('input, textarea, select')].map((control) =>
    'value' in control && typeof control.value === 'string' ? control.value : '',
  )
  return [copy.innerHTML, copy.textContent ?? '', ...values, document.title].join('\n')
}
