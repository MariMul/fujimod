// Step 1, "Start in service mode", in the owner's-manual line style (manual.ts):
// cable in, switch OFF, hold UP + shutter, switch ON, status light turns green.
// Motion follows the text-to-lottie motion guide: one camera move at a time with
// travel easing and a soft settle, arrows drawn on with trim paths, no bounce.
import { an, ellipse, fill, group, layer, nul, path, rect, rgb, scene, st, stroke, trim, type Json, type Layer, type Step } from './build.ts';
import { BLUE, DPAD, G, H, INK, PART, RED, SHUTTER_TOP, W, X, arrow, backView, segment, topView, type Pt } from './manual.ts';

const BG = '#f5f5f7';
const LED: Pt = [574, 334];
const PORT: Pt = [X.right, 444];
const LEVER = { on: 128, off: 58 };
const DRAW: [number, number, number, number] = [0.4, 0, 0.2, 1];

// ---------- timing (frames at 60 fps) ----------
const T = {
  b0: 0, cable: [44, 104],
  b1: 132, toTop: [132, 192], arrowOff: [200, 236], leverOff: [212, 244], screenOff: [232, 262], arrowOffOut: [276, 296],
  b2: 300, toAll: [300, 360], ringShutter: [368, 398], ringUp: [410, 440],
  b3: 470, toTop2: [470, 526], arrowOn: [534, 570], leverOn: [546, 578], arrowOnOut: [604, 624],
  b4: 630, toAll2: [630, 690], led: [700, 720], release: [724, 752],
  fade: [868, 896], op: 900,
} as const;

// ---------- the viewer's camera: everything hangs from this null ----------
interface View { a: number[]; p: number[]; s: number }
const ALL: View = { a: [480, 330, 0], p: [480, 320, 0], s: 96 };
const TOPV: View = { a: [600, 214, 0], p: [480, 320, 0], s: 168 };
const TOPV2: View = { a: [616, 290, 0], p: [480, 320, 0], s: 138 };
// view at each frame; moves travel between consecutive pairs, holds in between
const moves: [number, View][] = [[0, ALL], [T.toTop[0], ALL], [T.toTop[1], TOPV], [T.toAll[0], TOPV], [T.toAll[1], ALL], [T.toTop2[0], ALL], [T.toTop2[1], TOPV2], [T.toAll2[0], TOPV2], [T.toAll2[1], ALL]];
const track = (value: (v: View) => number[]): Json =>
  an(moves.map(([t, v], i): Step => [t, value(v), i === moves.length - 1 || i % 2 === 0 ? 'hold' : 'travel']));
const cam = nul('camera', {
  a: track((v) => v.a),
  p: track((v) => v.p),
  s: an([[0, [92, 92, 100], 'settle'], [44, [96, 96, 100], 'hold'], ...moves.slice(1).map(([t, v], i): Step => [t, [v.s, v.s, 100], i % 2 === 0 ? 'travel' : 'hold'])]),
  o: an([[0, 0, 'settle'], [30, 100, 'hold'], [T.fade[0], 100, 'exit'], [T.fade[1], 0]]),
}, { op: T.op });
const P = { parent: cam.ind, op: T.op };

// ---------- static drawing ----------
const top = layer('top-view', topView(), P);
const back = layer('back-view', backView(), P);

// ---------- animated parts ----------
const leverAngle = an([[0, LEVER.on, 'hold'], [T.leverOff[0], LEVER.on, 'travel'], [T.leverOff[1], LEVER.off, 'hold'], [T.leverOn[0], LEVER.off, 'travel'], [T.leverOn[1], LEVER.on]]);
const lever = layer('lever', [
  group('lever', [...path('M 18 -8 L 38 -6 C 45 -5, 45 5, 38 6 L 18 8 Z'), stroke(rgb(INK), PART), fill(rgb(G.light))], { p: SHUTTER_TOP, r: leverAngle }),
], P);
const indexMark = (angle: number): string => {
  const a = (angle * Math.PI) / 180, [cx, cy] = SHUTTER_TOP;
  return `M ${cx + 47 * Math.cos(a)} ${cy + 47 * Math.sin(a)} L ${cx + 54 * Math.cos(a)} ${cy + 54 * Math.sin(a)}`;
};
const marks = layer('indices', [group('on', [...path(indexMark(LEVER.on)), stroke(rgb(INK), 2.4)]), group('off', [...path(indexMark(LEVER.off)), stroke(rgb(INK), 2.4)])], P);

/** The head appears just as the drawn stroke reaches it. */
function revealHead(head: Json, t1: number) {
  const items = head.it as Json[];
  items[items.length - 1].o = an([[t1 - 8, 0, 'linear'], [t1 - 2, 100]]);
}
// red arrow around the collar: ON -> OFF, later OFF -> ON
function powerArrow(nm: string, from: number, to: number, [t0, t1]: readonly number[], [o0, o1]: readonly number[]): Layer {
  const r = 60, [cx, cy] = SHUTTER_TOP;
  const p = (a: number): Pt => [cx + r * Math.cos((a * Math.PI) / 180), cy + r * Math.sin((a * Math.PI) / 180)];
  const dir = (a: number): Pt => [-Math.sin((a * Math.PI) / 180), Math.cos((a * Math.PI) / 180)];
  const k = (4 / 3) * Math.tan(((to - from) * Math.PI) / 720) * r;
  const [x0, y0] = p(from), [x1, y1] = p(to), d0 = dir(from), d1 = dir(to);
  const [head, shaft] = arrow(nm, [x0, y0], [x0 + k * d0[0], y0 + k * d0[1]], [x1 - k * d1[0], y1 - k * d1[1]], [x1, y1], RED, 12, [trim(0, an([[t0, 0, DRAW], [t1, 100]]))]);
  revealHead(head, t1);
  return layer(nm, [head, shaft], { ...P, ip: t0 - 1, op: o1 + 1, ks: { o: an([[t0, 0, 'linear'], [t0 + 4, 100, 'hold'], [o0, 100, 'exit'], [o1, 0]]) } });
}
const offArrow = powerArrow('arrow-off', LEVER.on + 4, LEVER.off - 10, T.arrowOff, T.arrowOffOut);
const onArrow = powerArrow('arrow-on', LEVER.off - 10, LEVER.on + 4, T.arrowOn, T.arrowOnOut);

// blue "press and hold": arrow draws onto the key, ring draws around it, then breathes
function hold(nm: string, [cx, cy]: Pt, rx: number, ry: number, from: Pt, c1: Pt, c2: Pt, [t0, t1]: readonly number[]): Layer {
  const draw = (a: number, b: number) => trim(0, an([[a, 0, DRAW], [b, 100]]));
  const [head, shaft] = arrow(nm + '-arrow', from, c1, c2, [cx + rx * 0.62, cy - ry * 0.62], BLUE, 12, [draw(t0, t1)]);
  revealHead(head, t1);
  const ring = group('ring', [ellipse(cx, cy, rx * 2, ry * 2), { ...stroke(rgb(BLUE), 6), lc: 2 }, draw(t1 - 6, t1 + 22)]);
  const beats = [0, 1, 2, 3].map((n) => t1 + 22 + n * 61);
  const breathe = group('breathe', [ellipse(cx, cy, rx * 2, ry * 2), stroke(rgb(BLUE), 3)], {
    a: [cx, cy], p: [cx, cy],
    s: an(beats.flatMap((b, i): Step[] => [[b, [100, 100], 'linear'], [b + 60, [128, 128], i === beats.length - 1 ? 'linear' : 'hold']])),
    o: an(beats.flatMap((b, i): Step[] => [[b, 60, 'linear'], [b + 60, 0, i === beats.length - 1 ? 'linear' : 'hold']])),
  });
  return layer(nm, [head, shaft, ring, breathe], { ...P, ip: t0 - 1, op: T.release[1] + 1, ks: { o: an([[t0, 0, 'linear'], [t0 + 4, 100, 'hold'], [T.release[0], 100, 'exit'], [T.release[1], 0]]) } });
}
const holdShutter = hold('hold-shutter', SHUTTER_TOP, 30, 30, [SHUTTER_TOP[0] + 150, SHUTTER_TOP[1] - 112], [SHUTTER_TOP[0] + 104, SHUTTER_TOP[1] - 112], [SHUTTER_TOP[0] + 56, SHUTTER_TOP[1] - 82], T.ringShutter);
const UP: Pt = [DPAD[0], DPAD[1] - 33];
const holdUp = hold('hold-up', UP, 22, 16, [X.right + 96, DPAD[1] - 10], [X.right + 40, DPAD[1] - 40], [UP[0] + 70, UP[1] - 30], T.ringUp);

// keys tint light blue while held
const tint = (nm: string, shapes: Json[], t0: number): Layer => layer(nm, [group(nm, [...shapes, fill(rgb('#cfe3fb'))])], {
  ...P, ks: { o: an([[0, 0, 'hold'], [t0 + 18, 0, 'settle'], [t0 + 30, 100, 'hold'], [T.release[0], 100, 'exit'], [T.release[1], 0]]) },
});
const shutterTint = tint('shutter-tint', [ellipse(...SHUTTER_TOP, 24, 24)], T.ringShutter[0]);
const upTint = tint('up-tint', path(segment(DPAD[0], DPAD[1], 19, 45, -135, -45)), T.ringUp[0]);

// the screen goes dark when the camera switches off
const picture: Json = { ty: 'gf', nm: 'picture-fill', o: st(100), r: 1, s: st([238, 334]), e: st([534, 522]), t: 1, g: { p: 3, k: st([0, 0.62, 0.74, 0.86, 0.55, 0.86, 0.8, 0.66, 1, 0.55, 0.66, 0.56]) } };
const screenOn = layer('screen-on', [
  group('ui', [rect(254, 350, 64, 9, 4.5), rect(254, 496, 110, 7, 3.5), rect(468, 350, 46, 9, 4.5), fill(rgb('#ffffff'), 80)]),
  group('picture', [rect(238, 334, 296, 188, 6), stroke(rgb(INK), 1.4), picture]),
], { ...P, ks: { o: an([[T.screenOff[0], 100, 'exit'], [T.screenOff[1], 0, 'hold'], [T.op - 2, 0, 'hold'], [T.op - 1, 100]]) } });

// status light: turns green when service mode starts
const led = layer('led', [
  group('core', [ellipse(...LED, 11, 11), stroke(rgb(INK), 1.4), fill(rgb('#2fbf71'))]),
  group('glow', [ellipse(...LED, 26, 26), fill(rgb('#2fbf71'), 18)]),
], { ...P, ks: { a: [...LED, 0], p: [...LED, 0], o: an([[0, 0, 'hold'], [T.led[0], 0, 'settle'], [T.led[1], 100]]), s: an([[T.led[0], [60, 60, 100], 'settle'], [T.led[1], [100, 100, 100]]]) } });

// USB cable into the side port: round-capped lead with an outlined plug
const lead = `M ${PORT[0] + 34} ${PORT[1]} C ${PORT[0] + 110} ${PORT[1]}, ${PORT[0] + 120} ${PORT[1] + 130}, ${PORT[0] + 330} ${PORT[1] + 170}`;
const cable = layer('cable', [
  group('plug', [rect(PORT[0] + 2, PORT[1] - 13, 34, 26, 6), stroke(rgb(INK), PART), fill(rgb(G.light))]),
  group('plug-boot', [rect(PORT[0] + 26, PORT[1] - 9, 16, 18, 5), stroke(rgb(INK), PART), fill(rgb(G.mid))]),
  group('lead-fill', [...path(lead), { ...stroke(rgb(G.plate), 8), lc: 2 }]),
  group('lead-edge', [...path(lead), { ...stroke(rgb(INK), 8 + 2 * PART), lc: 2 }]),
], { ...P, ks: { p: an([[0, [320, 70, 0], 'hold'], [T.cable[0], [320, 70, 0], [0.3, 0, 0, 1]], [T.cable[1], [0, 0, 0]]]) } });
const port = layer('port', [group('port', [rect(PORT[0] - 4, PORT[1] - 10, 8, 20, 3), stroke(rgb(INK), PART), fill(rgb(G.dark))])], P);

const bg = layer('background', [group('bg', [rect(0, 0, W, H, 0), { ty: 'fl', nm: 'fill', c: { sid: 'bgColor', a: 0, k: rgb(BG) }, o: st(100), r: 1 }])], { op: T.op });

const layers: Layer[] = [offArrow, onArrow, holdShutter, holdUp, led, lever, marks, shutterTint, upTint, screenOn, port, top, back, cable, cam, bg];
for (const l of layers) l.op ??= T.op;

const beatsAt = [T.b0, T.b1, T.b2, T.b3, T.b4, T.op];
export default {
  ...scene({
    nm: 'Setup · Start in service mode', w: W, h: H, op: T.op, layers,
    markers: beatsAt.slice(0, -1).map((tm, i) => ({ tm, cm: `beat-${i}`, dr: beatsAt[i + 1] - tm })),
  }),
  slots: { bgColor: { p: { a: 0, k: rgb(BG) } } },
};
