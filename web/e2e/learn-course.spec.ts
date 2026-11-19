// Goal: at a desktop, a laptop and a phone with touch, every lesson of the Learn course is played to its end by one generic walker that presses the centre of what the coach lights (so a wrong highlight fails), after trying the inputs each do step holds back (the primary button, End turn, Space) and finding the lesson unmoved with the reason shown; the lesson never loses its place and ends on its last card.
import { expect, test } from '@playwright/test'
import { LAYOUTS, LESSONS, startLesson, walkLesson } from './learnWalker'

for (const layout of LAYOUTS) {
  test.describe(`the course at ${layout.name}`, () => {
    test.use({ hasTouch: layout.touch, isMobile: layout.touch })
    for (const lesson of LESSONS) {
      test(`${lesson} is played to its end by pressing what is lit`, async ({ page }) => {
        test.setTimeout(240_000)
        await startLesson(page, layout, lesson)
        const steps = await walkLesson(page, layout, { probe: true })
        expect(steps.at(-1)).toBe('end')
        expect(steps.length).toBeGreaterThan(4)
      })
    }
  })
}
