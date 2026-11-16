#!/usr/bin/env node
// Counts layout jumps of the table: steps a running dev server's /dev/table?script=layout through its states
// (web/src/features/table/dev/layoutScript.ts) in Chromium and, after each state settles, compares the boxes of the
// hand dock, the prompt band, the board rows, each board and the first card of each lane with the state before. A
// box that moves or resizes by more than 4 px is a jump.
// Usage: node scripts/layout-jumps.mjs [--base http://127.0.0.1:5173] [--tables duel,four,...] [--sizes 1873x1047,...]
//          [--json out.json] [--shots <dir>] [--detail]
// CHROMIUM_PATH picks the browser (on NixOS: CHROMIUM_PATH=$(ls -d /nix/store/*-chromium-[0-9]*/bin/chromium | head -n 1)).
import { execSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";

/** The tables measured, by name: their `/dev/table` parameters. */
const TABLES = {
  duel: "board=duel",
  four: "seats=4&permanents=20",
  grid6: "seats=6&mode=grid&permanents=16",
  teams: "seats=4&teams=2v2&permanents=20",
  "2hg": "seats=4&teams=2hg&permanents=20",
  boss: "seats=4&boss=1&permanents=16",
};
const SIZES = ["1873x1047", "1400x900", "390x844"];
/** A box moving or resizing by more than this many px is a jump. */
const JUMP_PX = 4;

function options(argv) {
  const config = {
    base: "http://127.0.0.1:5173",
    tables: Object.keys(TABLES),
    sizes: SIZES,
    json: null,
    shots: null,
    detail: false,
  };
  for (let at = 0; at < argv.length; at += 1) {
    const arg = argv[at];
    const value = () => {
      at += 1;
      return argv[at];
    };
    if (arg === "--base") config.base = value();
    else if (arg === "--tables") config.tables = value().split(",");
    else if (arg === "--sizes") config.sizes = value().split(",");
    else if (arg === "--json") config.json = value();
    else if (arg === "--shots") config.shots = value();
    else if (arg === "--detail") config.detail = true;
    else throw new Error(`unknown option ${arg}`);
  }
  return config;
}

function chromiumPath() {
  if (process.env.CHROMIUM_PATH) return process.env.CHROMIUM_PATH;
  try {
    return execSync(
      "ls -d /nix/store/*-chromium-[0-9]*/bin/chromium | head -n 1",
      { stdio: ["ignore", "pipe", "ignore"] },
    )
      .toString()
      .trim();
  } catch {
    return undefined;
  }
}

/** In the page: the boxes of the tracked elements, by key and group. */
function look() {
  const boxes = {};
  const put = (group, key, element) => {
    if (element === null || element === undefined) return;
    const r = element.getBoundingClientRect();
    if (r.width === 0 && r.height === 0) return;
    boxes[`${group}:${key}`] = [r.x, r.y, r.width, r.height];
  };
  put("hand", "dock", document.querySelector('[data-region="hand"]'));
  put("band", "strip", document.querySelector('[data-region="shared"]'));
  put("band", "sheet", document.querySelector('[data-slot="shared-sheet"]'));
  for (const area of ["opponents", "own"])
    put("row", area, document.querySelector(`[data-area="${area}"]`));
  for (const board of document.querySelectorAll("[data-battlefield-seat]")) {
    const seat = board.getAttribute("data-battlefield-seat");
    put("board", seat, board);
    for (const lane of board.querySelectorAll("[data-lane]")) {
      put(
        "card",
        `${seat}:${lane.getAttribute("data-lane")}`,
        lane.querySelector("button[data-object-id]"),
      );
    }
  }
  return boxes;
}

/** In the page: waits until the boxes hold still for six frames, at most two seconds. */
async function settle() {
  const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
  const key = () => {
    const parts = [];
    for (const el of document.querySelectorAll(
      "[data-region], [data-area], [data-battlefield-seat], [data-lane]",
    )) {
      const r = el.getBoundingClientRect();
      parts.push(
        `${Math.round(r.x)},${Math.round(r.y)},${Math.round(r.width)},${Math.round(r.height)}`,
      );
    }
    return parts.join("|");
  };
  const start = performance.now();
  let last = "";
  let still = 0;
  let changes = -1;
  while (performance.now() - start < 2000) {
    await frame();
    const now = key();
    if (now === last) still += 1;
    else {
      still = 0;
      changes += 1;
      last = now;
    }
    if (still >= 6) break;
  }
  return changes;
}

const moved = (a, b) =>
  a.some((value, index) => Math.abs(value - b[index]) > JUMP_PX);

const config = options(process.argv.slice(2));
const { chromium } = createRequire(
  new URL("../web/package.json", import.meta.url),
)("@playwright/test");
const browser = await chromium.launch({
  executablePath: chromiumPath() || undefined,
});
const results = [];
try {
  for (const table of config.tables) {
    for (const size of config.sizes) {
      const [width, height] = size.split("x").map(Number);
      const page = await browser.newPage({ viewport: { width, height } });
      await page.goto(
        `${config.base}/dev/table?${TABLES[table]}&script=layout`,
      );
      await page.waitForFunction(
        () => window.__tableScript !== undefined,
        null,
        { timeout: 30_000 },
      );
      const count = await page.evaluate(() => window.__tableScript.count);
      const names = await page.evaluate(() => window.__tableScript.names);
      await page.evaluate(() => window.__tableScript.go(0));
      await page.evaluate(settle);
      let before = await page.evaluate(look);
      const groups = { band: 0, hand: 0, row: 0, board: 0, card: 0 };
      let passes = 0;
      const detail = [];
      for (let index = 1; index < count; index += 1) {
        await page.evaluate((at) => window.__tableScript.go(at), index);
        passes += Math.max(0, (await page.evaluate(settle)) - 1);
        const after = await page.evaluate(look);
        const jumped = Object.keys(after).filter(
          (key) => key in before && moved(before[key], after[key]),
        );
        for (const key of jumped) groups[key.split(":")[0]] += 1;
        const sized = (key) =>
          moved(before[key].slice(2), after[key].slice(2))
            ? `${key}(size)`
            : key;
        if (jumped.length > 0)
          detail.push(
            `${index} ${names[index]}: ${jumped.map(sized).join(" ")}`,
          );
        if (config.shots !== null) {
          mkdirSync(config.shots, { recursive: true });
          await page.screenshot({
            path: `${config.shots}/${table}-${size}-${String(index).padStart(2, "0")}.png`,
          });
        }
        before = after;
      }
      const total = Object.values(groups).reduce((a, b) => a + b, 0);
      results.push({ table, size, total, passes, ...groups });
      console.log(
        `${table.padEnd(6)} ${size.padEnd(10)} jumps ${String(total).padStart(4)}  band ${groups.band} hand ${groups.hand} rows ${groups.row} boards ${groups.board} cards ${groups.card}  extra passes ${passes}`,
      );
      if (config.detail) for (const line of detail) console.log(`    ${line}`);
      await page.close();
    }
  }
} finally {
  await browser.close();
}
if (config.json !== null)
  writeFileSync(config.json, JSON.stringify(results, null, 2));
