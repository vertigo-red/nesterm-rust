// Development only. Use the unmodified upstream checkout as an independent
// oracle; Node is not required to build, test, install, or run nesterm-rust.
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { writeFileSync } from 'node:fs';

const root = resolve(process.argv[2] ?? '../nesterm-upstream');
const { AsciiRenderer } = await import(pathToFileURL(`${root}/src/renderer.mjs`));
const scenarios = ['black', 'white', 'blue', 'patch', 'shift1', 'shift4', 'overscan', 'noise', 'black'];

function pixels(kind) {
  const out = new Uint32Array(256 * 240);
  if (kind === 'white') out.fill(0xffffff);
  if (kind === 'blue') out.fill(0x2070e0);
  if (['patch', 'shift1', 'shift4'].includes(kind)) {
    const shift = kind === 'shift1' ? 1 : kind === 'shift4' ? 4 : 0;
    for (let y = 72; y < 112; y++) for (let x = 64 + shift; x < 112 + shift; x++)
      out[y * 256 + x] = x % 4 < 2 ? 0x2070e0 : 0xe08020;
    for (let y = 91; y < 103; y++) for (let x = 178 + shift; x < 182 + shift; x++)
      out[y * 256 + x] = 0xffffff;
  }
  if (kind === 'overscan') { out.fill(0x88eeaa); out.fill(0, 210 * 256); }
  if (kind === 'noise') {
    let state = 0x5eed;
    for (let i = 0; i < out.length; i++) {
      state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
      out[i] = state & 0xffffff;
    }
  }
  return out;
}

function hashColors(colors) {
  let hash = 2166136261;
  for (const c of colors) for (let shift = 0; shift < 32; shift += 8)
    hash = Math.imul(hash ^ ((c >>> shift) & 255), 16777619) >>> 0;
  return hash;
}

const groups = [];
for (const [cols, rows] of [[40, 25], [64, 30]]) for (const mode of ['shape', 'ramp']) for (const color of [true, false]) {
  const renderer = new AsciiRenderer({ cols, rows, mode, color });
  const frames = scenarios.map(kind => {
    const frame = renderer.render(pixels(kind));
    return { kind, chars: frame.chars, colorsHash: hashColors(frame.colors) };
  });
  groups.push({ cols, rows, mode, color, frames });
}
writeFileSync('tests/fixtures/renderer-golden.json', JSON.stringify({
  source: 'kathoc/nesterm@fd3b83e159fdfcd1497b9ed5dbaa4854d1f17b1a', groups,
}) + '\n');
