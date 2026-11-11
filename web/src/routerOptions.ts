/**
 * How the app's router loads its pages: the code of the page a link leads to is fetched as soon as the pointer rests on
 * the link or it takes the focus, so the click opens a page whose code is already there.
 */
export const ROUTER_OPTIONS = {
  defaultPreload: 'intent',
  defaultPreloadDelay: 50,
} as const
