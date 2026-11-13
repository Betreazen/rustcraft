'use strict';
// Textures and the scene model of the RustCraft engineer's guide. Scenes and block icons are drawn
// in 3D by player.js.
// Scene axes: x and y are horizontal, z is up, and the default view looks from +x, +y, +z, where
// x runs right-down and y runs left-down. One block is 16 texture pixels.
(function () {
  const RC = (window.RC = {});

  RC.hash = (s) => {
    let h = 2166136261;
    for (let i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h = Math.imul(h, 16777619);
    }
    return h >>> 0;
  };
  RC.rng = (seed) => {
    let s = seed >>> 0 || 1;
    return () => (s = (Math.imul(s, 1664525) + 1013904223) >>> 0) / 4294967296;
  };

  const hex2rgb = (h) => {
    const n = parseInt(h.slice(1), 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  };
  const clamp255 = (v) => Math.max(0, Math.min(255, Math.round(v)));
  const rgb2hex = (r, g, b) =>
    '#' + ((1 << 24) | (clamp255(r) << 16) | (clamp255(g) << 8) | clamp255(b)).toString(16).slice(1);
  // f > 0 lightens towards white, f < 0 darkens towards black.
  RC.shade = (h, f) => {
    const [r, g, b] = hex2rgb(h);
    return f >= 0
      ? rgb2hex(r + (255 - r) * f, g + (255 - g) * f, b + (255 - b) * f)
      : rgb2hex(r * (1 + f), g * (1 + f), b * (1 + f));
  };

  // ---------------------------------------------------------------- textures
  const TEX = {};
  const texCache = {};
  const texMakers = [];
  RC.defTex = (name, def) => {
    TEX[name] = def;
  };
  // A maker turns a texture name with a known prefix into a definition on demand.
  RC.texMaker = (prefix, fn) => texMakers.push([prefix, fn]);

  const px = (g, x, y, col) => {
    g.fillStyle = col;
    g.fillRect(x, y, 1, 1);
  };
  const rect = (g, x, y, w, h, col) => {
    g.fillStyle = col;
    g.fillRect(x, y, w, h);
  };
  RC.px = px;
  RC.rect = rect;

  const DARK = '#15171c';
  const FEATURES = {
    vent: (g) => [5, 7, 9, 11].forEach((y) => rect(g, 4, y, 8, 1, DARK)),
    window: (g, d, a) => {
      rect(g, 4, 4, 8, 8, DARK);
      rect(g, 5, 5, 6, 6, RC.shade(a, -0.45));
      rect(g, 5, 5, 3, 2, a);
      px(g, 10, 10, RC.shade(a, 0.4));
    },
    screen: (g, d, a) => {
      rect(g, 3, 3, 10, 7, DARK);
      rect(g, 4, 4, 8, 5, RC.shade(a, -0.55));
      rect(g, 4, 5, 5, 1, a);
      rect(g, 4, 7, 7, 1, a);
      px(g, 4, 12, '#e0564a');
      px(g, 6, 12, '#59d38c');
      px(g, 8, 12, '#ffd24a');
    },
    coil: (g, d, a) =>
      [4, 7, 10].forEach((y) => {
        rect(g, 3, y, 10, 1, a);
        rect(g, 3, y + 1, 10, 1, RC.shade(a, -0.35));
      }),
    grid: (g, d, a) => {
      for (let i = 0; i < 3; i++)
        for (let j = 0; j < 3; j++) {
          rect(g, 3 + 4 * i, 3 + 4 * j, 3, 3, DARK);
          px(g, 4 + 4 * i, 4 + 4 * j, a);
        }
    },
    drawer: (g, d, a) =>
      [3, 9].forEach((y) => {
        rect(g, 2, y, 12, 4, RC.shade(d.base, -0.3));
        rect(g, 3, y + 1, 10, 2, RC.shade(d.base, -0.1));
        rect(g, 6, y + 2, 4, 1, a);
      }),
    port: (g, d, a) => {
      rect(g, 5, 5, 6, 6, RC.shade(d.base, 0.3));
      rect(g, 6, 6, 4, 4, DARK);
      rect(g, 7, 7, 2, 2, RC.shade(a, -0.2));
    },
    jaws: (g, d) => {
      rect(g, 3, 5, 10, 6, DARK);
      for (let x = 3; x < 13; x++) {
        px(g, x, 5 + (x % 2), RC.shade(d.base, 0.45));
        px(g, x, 10 - (x % 2), RC.shade(d.base, 0.45));
      }
    },
    flame: (g, d, a) => {
      rect(g, 4, 8, 8, 5, DARK);
      rect(g, 5, 9, 6, 4, a);
      rect(g, 6, 10, 3, 3, RC.shade(a, 0.45));
      px(g, 9, 9, RC.shade(a, -0.3));
    },
    cells: (g, d, a) => {
      for (let i = 0; i < 3; i++)
        for (let j = 0; j < 3; j++) {
          rect(g, 1 + 5 * i, 1 + 5 * j, 4, 4, RC.shade(a, -0.15));
          px(g, 1 + 5 * i, 1 + 5 * j, RC.shade(a, 0.45));
        }
    },
    bars: (g) => [4, 7, 10].forEach((x) => rect(g, x, 3, 2, 10, DARK)),
    lens: (g, d, a) => {
      rect(g, 5, 4, 6, 8, DARK);
      rect(g, 4, 5, 8, 6, DARK);
      rect(g, 6, 5, 4, 6, a);
      rect(g, 5, 6, 6, 4, a);
      px(g, 7, 6, '#ffffff');
    },
    stripe: (g, d, a) => {
      rect(g, 0, 11, 16, 3, a);
      for (let x = 0; x < 16; x += 4) rect(g, x, 11, 2, 3, DARK);
    },
    arrows: (g, d, a) => {
      for (let i = 0; i < 3; i++) {
        const x = 3 + 4 * i;
        [[0, 5], [1, 6], [2, 7], [2, 8], [1, 9], [0, 10]].forEach(([dx, y]) => px(g, x + dx, y, a));
      }
    },
    ring: (g, d, a) => {
      rect(g, 3, 3, 10, 1, a);
      rect(g, 3, 12, 10, 1, a);
      rect(g, 3, 3, 1, 10, a);
      rect(g, 12, 3, 1, 10, a);
    },
    facets: (g, d) => {
      for (let i = 0; i < 16; i++) {
        px(g, i, (i * 2 + 3) % 16, RC.shade(d.base, 0.5));
        px(g, (i + 5) % 16, i, RC.shade(d.base, 0.28));
        px(g, (i + 11) % 16, i, RC.shade(d.base, -0.25));
      }
    },
    planks: (g, d) => [3, 7, 11, 15].forEach((y) => rect(g, 0, y, 16, 1, RC.shade(d.base, -0.3))),
    bark: (g, d) => [2, 6, 9, 13].forEach((x) => rect(g, x, 0, 1, 16, RC.shade(d.base, -0.28))),
    rows: (g, d) => [2, 6, 10, 14].forEach((y) => rect(g, 0, y, 16, 1, RC.shade(d.base, -0.25))),
    latch: (g) => {
      rect(g, 7, 6, 2, 3, '#c9ccd4');
      rect(g, 0, 5, 16, 1, DARK);
    },
  };

  function paintTexture(g, d, seed) {
    const r = RC.rng(seed);
    const n = d.noise == null ? 0.07 : d.noise;
    for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) px(g, x, y, RC.shade(d.base, (r() - 0.5) * 2 * n));
    for (const [col, count, big] of d.speck || []) {
      for (let i = 0; i < count; i++) {
        const x = (r() * 14) | 0;
        const y = (r() * 14) | 0;
        px(g, x + 1, y + 1, col);
        if (big) {
          px(g, x + 2, y + 1, col);
          px(g, x + 1, y + 2, RC.shade(col, -0.25));
        }
      }
    }
    if (d.frame) {
      const dk = RC.shade(d.base, -0.38);
      const lt = RC.shade(d.base, 0.28);
      rect(g, 0, 0, 16, 1, lt);
      rect(g, 0, 0, 1, 16, lt);
      rect(g, 0, 15, 16, 1, dk);
      rect(g, 15, 0, 1, 16, dk);
      [[2, 2], [13, 2], [2, 13], [13, 13]].forEach(([x, y]) => px(g, x, y, dk));
    }
    if (d.trim) rect(g, 1, 1, 14, 1, d.trim);
    const a = d.accent || '#ffd24a';
    for (const f of [].concat(d.feat || [])) FEATURES[f] && FEATURES[f](g, d, a);
    if (d.paint) d.paint(g, d);
  }

  // ---------------------------------------------------------------- vanilla textures
  // tools/local-mc-assets/extract.py copies the textures of the base game out of the owner's own
  // game client into local/mc-assets.js, which never goes into git:
  // window.RC_MC = { 'block/stone': [width, height, base64 RGBA], ... }. Without that file every
  // texture falls back to its hand-drawn definition.
  const mcCache = {};
  // The whole texture as a canvas, or null when the client did not provide it.
  RC.mcImage = (path) => {
    if (path in mcCache) return mcCache[path];
    const src = window.RC_MC && window.RC_MC[path];
    let c = null;
    if (src) {
      const [w, h, b64] = src;
      c = document.createElement('canvas');
      c.width = w;
      c.height = h;
      const g = c.getContext('2d');
      const img = g.createImageData(w, h);
      const bin = atob(b64);
      for (let i = 0; i < bin.length && i < img.data.length; i++) img.data[i] = bin.charCodeAt(i);
      g.putImageData(img, 0, 0);
    }
    return (mcCache[path] = c);
  };
  // def.mc is a texture path, or { src, at: [x, y, w, h], over: [dx, dy], flip, scale, size }: the
  // region `at` of the texture `src`, with the region shifted by `over` drawn on top of it (the
  // outer layer of a skin), turned upside down when `flip`, at `scale` canvas pixels to a texture
  // pixel, in the top left corner of a square canvas `size` pixels wide. A canvas spans one block.
  function mcTexture(spec) {
    if (typeof spec === 'string') return RC.mcImage(spec);
    const img = RC.mcImage(spec.src);
    if (!img) return null;
    const [x, y, w, h] = spec.at, k = spec.scale || 1;
    const c = document.createElement('canvas');
    c.width = c.height = spec.size || 16;
    const g = c.getContext('2d');
    g.imageSmoothingEnabled = false;
    if (spec.flip) {
      g.translate(0, h * k);
      g.scale(1, -1);
    }
    g.drawImage(img, x, y, w, h, 0, 0, w * k, h * k);
    if (spec.over) g.drawImage(img, x + spec.over[0], y + spec.over[1], w, h, 0, 0, w * k, h * k);
    return c;
  }

  // A definition may name a vanilla texture (mc) or build its canvas itself (make, which returns
  // null when it cannot); its hand-drawn fields are the fallback.
  RC.tex = (name) => {
    if (texCache[name]) return texCache[name];
    let def = TEX[name];
    if (!def) {
      const m = texMakers.find(([p]) => name.startsWith(p));
      def = m ? m[1](name.slice(m[0].length)) : { base: '#ff00ff' };
    }
    const own = (def.mc && mcTexture(def.mc)) || (def.make && def.make());
    if (own) return (texCache[name] = own);
    const c = document.createElement('canvas');
    c.width = c.height = 16;
    paintTexture(c.getContext('2d'), def, RC.hash(name));
    return (texCache[name] = c);
  };

  // ---------------------------------------------------------------- boxes
  // A box may carry rot: { axis: 'x' | 'y' | 'z', at: [x, y, z], angle }. It is then turned about
  // the line through `at` that runs along the axis, by the right-hand rule in scene numbers.
  const TURN = { x: [1, 2], y: [2, 0], z: [0, 1] };
  // Turns the point `p` in place about the centre `c`; pass [0, 0, 0] to turn a direction.
  function turn(p, rot, c) {
    const [i, j] = TURN[rot.axis];
    const sin = Math.sin(rot.angle), cos = Math.cos(rot.angle);
    const a = p[i] - c[i], b = p[j] - c[j];
    p[i] = c[i] + a * cos - b * sin;
    p[j] = c[j] + a * sin + b * cos;
    return p;
  }
  function corners(b) {
    const pts = [];
    for (let n = 0; n < 8; n++) {
      const p = [b.x + (n & 1) * b.w, b.y + ((n >> 1) & 1) * b.d, b.z + (n >> 2) * b.h];
      pts.push(b.rot ? turn(p, b.rot, b.rot.at) : p);
    }
    return pts;
  }

  // ---------------------------------------------------------------- scene
  const MODELS = (RC.MODELS = {});

  function pathLength(pts) {
    let len = 0;
    for (let i = 1; i < pts.length; i++) len += Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1], pts[i][2] - pts[i - 1][2]);
    return len;
  }
  // Returns [x, y, z, tag] at distance `dist` along the polyline; the tag comes from the segment start.
  function pointOnPath(pts, dist) {
    for (let i = 1; i < pts.length; i++) {
      const a = pts[i - 1], b = pts[i];
      const seg = Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]);
      if (dist <= seg || i === pts.length - 1) {
        const f = seg ? Math.min(1, dist / seg) : 0;
        return [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f, a[3], Math.sign(b[0] - a[0]), Math.sign(b[1] - a[1])];
      }
      dist -= seg;
    }
    return pts[0];
  }

  class Scene {
    constructor() {
      this.objs = new Map();
      this.flows = [];
      this.beams = [];
      this.badges = [];
      this.tags = [];
      this.tint = null;
      this.now = 0;
    }
    put(id, type, x, y, z, props) {
      this.objs.set(id, Object.assign({ id, type, x, y, z }, props));
      return this;
    }
    // Places a row of identical objects: fill('s', 'stone', 0, 0, 0, 3, 1) puts s0..s2 along x.
    fill(id, type, x, y, z, nx, ny, props) {
      for (let i = 0; i < nx; i++) for (let j = 0; j < ny; j++) this.put(id + i + '_' + j, type, x + i, y + j, z, props);
      return this;
    }
    del(...ids) {
      ids.forEach((id) => this.objs.delete(id));
      return this;
    }
    set(id, props) {
      const o = this.objs.get(id);
      if (o) Object.assign(o, props);
      return this;
    }
    // Moves an object along a polyline. mode: 'once', 'loop' or 'ping'.
    move(id, pts, speed, mode) {
      return this.set(id, { path: pts, speed: speed || 2, mode: mode || 'once', t0: this.now });
    }
    flow(id, pts, opts) {
      this.flows = this.flows.filter((f) => f.id !== id);
      if (pts) this.flows.push(Object.assign({ id, pts, color: '#ffb347', n: 3, speed: 1.6 }, opts));
      return this;
    }
    // A beam from point a to point b, or to the object with id b, which it then follows.
    beam(id, a, b, color) {
      this.beams = this.beams.filter((f) => f.id !== id);
      if (a) this.beams.push({ id, a, b, color: color || '#ff5a4a' });
      return this;
    }
    badge(id, x, y, z, text, kind) {
      this.badges = this.badges.filter((f) => f.id !== id);
      if (text) this.badges.push({ id, x, y, z, text, kind: kind || 'bad' });
      return this;
    }
    position(o, t) {
      if (!o.path) return [o.x, o.y, o.z, o.mount];
      const len = pathLength(o.path);
      let dist = (t - o.t0) * o.speed;
      if (o.mode === 'loop') dist %= len;
      else if (o.mode === 'ping') {
        dist %= 2 * len;
        if (dist > len) {
          // On the way back the object faces the other way.
          const p = pointOnPath(o.path, 2 * len - dist);
          p[4] = -p[4];
          p[5] = -p[5];
          return p;
        }
      } else dist = Math.min(dist, len);
      return pointOnPath(o.path, dist);
    }
    // Collects the boxes of every object for the moment `t`.
    build(t) {
      const boxes = [];
      const out = {
        box(o, dx, dy, dz, w, d, h, top, left, right, extra) {
          const b = Object.assign({ x: o.px + dx, y: o.py + dy, z: o.pz + dz, w, d, h, top, left: left || top, right: right || left || top, owner: o }, extra);
          // The pivot of a turned box is given in block coordinates, like the box itself.
          if (b.rot) b.rot = { axis: b.rot.axis, angle: b.rot.angle, at: [o.px + b.rot.at[0], o.py + b.rot.at[1], o.pz + b.rot.at[2]] };
          boxes.push(b);
        },
      };
      for (const o of this.objs.values()) {
        const p = this.position(o, t);
        o.px = p[0];
        o.py = p[1];
        o.pz = p[2];
        o.tag = p[3];
        if (p[4] || p[5]) {
          o.hx = p[4];
          o.hy = p[5];
        }
        const m = MODELS[o.type];
        if (m) m(o, t, out);
      }
      return boxes;
    }
  }
  RC.Scene = Scene;

  // ---------------------------------------------------------------- exports
  // Shared with player.js.
  Object.assign(RC, { DARK, hex2rgb, turn, corners, pathLength, pointOnPath });
})();
