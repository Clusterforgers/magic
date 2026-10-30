/** A page with boxes and animations the test controls, for the travel layer: jsdom has no layout and no Web Animations API. */
import { vi } from 'vitest'
import type { FrameScheduler } from '@/features/table/fx/travel'
import { fakeFrames } from './frames'
import type { FakeFrames } from './frames'

/** A box on screen. */
export interface TestBox {
  x: number
  y: number
  width: number
  height: number
}

/** Puts a `tag` with `attrs` into `parent` and gives it `box` as the box it measures. */
export function place(
  parent: Element,
  attrs: Record<string, string>,
  box: TestBox,
  tag: keyof HTMLElementTagNameMap = 'div',
): HTMLElement {
  const element = document.createElement(tag)
  for (const [name, value] of Object.entries(attrs)) element.setAttribute(name, value)
  setBox(element, box)
  parent.append(element)
  return element
}

/** Gives `element` the box it measures from now on. */
export function setBox(element: Element, { x, y, width, height }: TestBox): void {
  element.getBoundingClientRect = () =>
    ({
      x,
      y,
      left: x,
      top: y,
      right: x + width,
      bottom: y + height,
      width,
      height,
      toJSON: () => ({}),
    }) as DOMRect
}

/** Frames the travel layer asks for that run when the test says so. */
export function travelFrames(): FakeFrames & { scheduler: FrameScheduler } {
  const frames = fakeFrames()
  const scheduler: FrameScheduler = (callback) => {
    const handle = frames.frames.request(callback)
    return () => frames.frames.cancel(handle)
  }
  return { ...frames, scheduler }
}

/** One animation a test started on an element. */
export interface FakeAnimation {
  element: Element
  keyframes: Keyframe[]
  timing: KeyframeAnimationOptions
  cancel: ReturnType<typeof vi.fn<() => void>>
  onfinish: (() => void) | null
  /** Ends the animation as the browser does. */
  finish(): void
}

/** `Element.animate` as a recorder; `restore` puts the page back as it was. */
export function fakeAnimate(): { started: FakeAnimation[]; live(): FakeAnimation[]; restore(): void } {
  const started: FakeAnimation[] = []
  const had = Object.getOwnPropertyDescriptor(Element.prototype, 'animate')
  Object.defineProperty(Element.prototype, 'animate', {
    configurable: true,
    writable: true,
    value(this: Element, keyframes: Keyframe[], timing: KeyframeAnimationOptions) {
      const animation: FakeAnimation = {
        element: this,
        keyframes,
        timing,
        cancel: vi.fn(),
        onfinish: null,
        finish: () => animation.onfinish?.(),
      }
      started.push(animation)
      return animation
    },
  })
  return {
    started,
    live: () => started.filter((animation) => animation.cancel.mock.calls.length === 0),
    restore() {
      if (had === undefined) Reflect.deleteProperty(Element.prototype, 'animate')
      else Object.defineProperty(Element.prototype, 'animate', had)
    },
  }
}
