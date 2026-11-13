'use strict';
// 3D scene player of the guide: a WebGL stage drawn with three.js, Blender models and the Player.
// Everything here uses scene numbers (x and y horizontal, z up); see engine.js.
(function () {
  const { DARK, hex2rgb, turn, corners, pathLength, pointOnPath, rect, Scene } = RC;
  const ZERO = [0, 0, 0];
  const EPS = 1e-6;

  // ---------------------------------------------------------------- 3D stage
  // Everything in a scene goes through one flat shader: a texture or a plain colour, darkened by
  // the side the face looks to (top 0, y sides 0.22, x sides 0.42, as on the icons) and lightened
  // by glow. There are no lights, so boxes and Blender models share one look.
  const LW = 560, LH = 340;
  const VERT = 13; // floats per vertex: position 3, normal 3, uv 2, tone 4 (colour and alpha), glow 1
  const VERTEX_SHADER = `
    attribute vec4 tone;
    attribute float glow;
    varying vec2 vUv;
    varying vec4 vTone;
    varying float vGlow;
    varying float vShade;
    void main() {
      vec3 n = normalize(mat3(modelMatrix) * normal);
      vShade = 0.42 * n.x * n.x + 0.22 * n.z * n.z + (n.y < 0.0 ? 0.5 * n.y * n.y : 0.0);
      vUv = uv;
      vTone = tone;
      vGlow = glow;
      gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    }`;
  const FRAGMENT_SHADER = `
    uniform sampler2D map;
    varying vec2 vUv;
    varying vec4 vTone;
    varying float vGlow;
    varying float vShade;
    void main() {
      vec3 c = texture2D(map, vUv).rgb * vTone.rgb;
      c = mix(c, vec3(0.031, 0.039, 0.086), vShade);
      c = mix(c, vec3(1.0), vGlow);
      gl_FragColor = vec4(c, vTone.a);
    }`;
  // solid: opaque boxes and models. clear: see-through boxes. over: flow dots and beams, which
  // show through everything.
  const KINDS = { solid: {}, clear: { transparent: true, depthWrite: false }, over: { transparent: true, depthTest: false } };

  const TONES = {};
  const toneOf = (h) => TONES[h] || (TONES[h] = hex2rgb(h).map((v) => v / 255));
  const WHITE = [1, 1, 1];
  const UP = [0, 0, 1];
  const P = [0, 0, 0], N = [0, 0, 0]; // scratch point and normal
  // Scratch matrices for posing Blender models. Turns follow the right-hand rule in scene
  // numbers, as box turns in engine.js do.
  const WHOLE = new THREE.Matrix4(), SHIFT = new THREE.Matrix4(), SPIN = new THREE.Matrix4();
  const rotation = (axis, angle) => (axis === 'x' ? SPIN.makeRotationX(angle) : axis === 'y' ? SPIN.makeRotationY(angle) : SPIN.makeRotationZ(angle));

  // A growing triangle list drawn with one material.
  class Batch {
    constructor(src, kind) {
      let image = src && RC.tex(src);
      if (!image) {
        image = document.createElement('canvas');
        image.width = image.height = 1;
        rect(image.getContext('2d'), 0, 0, 1, 1, '#ffffff');
      }
      const map = new THREE.CanvasTexture(image);
      map.magFilter = map.minFilter = THREE.NearestFilter;
      map.wrapS = map.wrapT = THREE.RepeatWrapping;
      map.generateMipmaps = false;
      const material = new THREE.ShaderMaterial(Object.assign({ uniforms: { map: { value: map } }, vertexShader: VERTEX_SHADER, fragmentShader: FRAGMENT_SHADER }, KINDS[kind]));
      this.mesh = new THREE.Mesh(new THREE.BufferGeometry(), material);
      this.mesh.frustumCulled = false;
      if (kind === 'over') this.mesh.renderOrder = 2;
      this.n = 0;
      this.grow(288);
    }
    grow(verts) {
      const old = this.data;
      this.data = new Float32Array(verts * VERT);
      if (old) this.data.set(old);
      this.buffer = new THREE.InterleavedBuffer(this.data, VERT).setUsage(THREE.DynamicDrawUsage);
      this.mesh.geometry.dispose();
      const geo = (this.mesh.geometry = new THREE.BufferGeometry());
      let at = 0;
      for (const [name, size] of [['position', 3], ['normal', 3], ['uv', 2], ['tone', 4], ['glow', 1]]) {
        geo.setAttribute(name, new THREE.InterleavedBufferAttribute(this.buffer, size, at));
        at += size;
      }
    }
    add(p, n, u, v, r, g, b, a, glow) {
      if (this.n * VERT === this.data.length) this.grow(this.n * 2);
      const d = this.data;
      let i = this.n++ * VERT;
      d[i++] = p[0]; d[i++] = p[1]; d[i++] = p[2];
      d[i++] = n[0]; d[i++] = n[1]; d[i++] = n[2];
      d[i++] = u; d[i++] = v;
      d[i++] = r; d[i++] = g; d[i++] = b; d[i++] = a;
      d[i] = glow;
    }
    end() {
      this.buffer.needsUpdate = true;
      this.mesh.geometry.setDrawRange(0, this.n);
      this.mesh.visible = this.n > 0;
    }
  }

  // The faces of a box as seen from outside: normal, texture slot, corners from the top left
  // clockwise (0 is the low end of the box on that axis, 1 the high end), and the two box sizes
  // the texture runs along. The texture starts in the top left corner, 16 pixels to a block.
  const FACES = [
    { n: [0, 0, 1], src: 'top', at: [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]], u: 'w', v: 'd' },
    { n: [0, 0, -1], src: 'top', at: [[0, 1, 0], [1, 1, 0], [1, 0, 0], [0, 0, 0]], u: 'w', v: 'd' },
    { n: [0, 1, 0], src: 'left', at: [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]], u: 'w', v: 'h' },
    { n: [0, -1, 0], src: 'left', at: [[1, 0, 1], [0, 0, 1], [0, 0, 0], [1, 0, 0]], u: 'w', v: 'h' },
    { n: [1, 0, 0], src: 'right', at: [[1, 1, 1], [1, 0, 1], [1, 0, 0], [1, 1, 0]], u: 'd', v: 'h' },
    { n: [-1, 0, 0], src: 'right', at: [[0, 0, 1], [0, 1, 1], [0, 1, 0], [0, 0, 0]], u: 'd', v: 'h' },
  ];
  const QUAD = [0, 1, 2, 0, 2, 3];

  // The one WebGL canvas of the page. Players take turns on it: only one scene is on screen.
  class Stage {
    constructor() {
      this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
      this.canvas = this.renderer.domElement;
      this.canvas.setAttribute('role', 'img');
      this.canvas.setAttribute('aria-label', 'Анимированная сцена. Её содержание описано в подписи под ней.');
      this.scene = new THREE.Scene();
      // Scene numbers are left-handed and z-up, three.js is right-handed and y-up. Swapping y and
      // z here converts everything below this group, so the rest of the code uses scene numbers.
      this.root = new THREE.Group();
      this.root.matrixAutoUpdate = false;
      this.root.matrix.set(1, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 1);
      this.root.matrixWorldNeedsUpdate = true;
      this.scene.add(this.root);
      this.batches = new Map();
      // Night: a veil over the whole picture, drawn after the blocks and before flows and beams.
      this.tint = new THREE.Mesh(
        new THREE.PlaneGeometry(2, 2),
        new THREE.ShaderMaterial({
          uniforms: { color: { value: new THREE.Vector4() } },
          vertexShader: 'void main() { gl_Position = vec4(position.xy, 0.0, 1.0); }',
          fragmentShader: 'uniform vec4 color; void main() { gl_FragColor = color; }',
          transparent: true,
          depthTest: false,
        })
      );
      this.tint.frustumCulled = false;
      this.tint.renderOrder = 1;
      this.scene.add(this.tint);
      // White edges around the object under the pointer.
      this.outline = new THREE.Group();
      this.outline.renderOrder = 3;
      this.root.add(this.outline);
      this.edges = new THREE.EdgesGeometry(new THREE.BoxGeometry(1, 1, 1));
      this.line = new THREE.LineBasicMaterial({ color: 0xffffff, transparent: true });
    }
    resize(w, h) {
      const ratio = window.devicePixelRatio || 1;
      if (w === this.w && h === this.h && ratio === this.ratio) return;
      this.w = w;
      this.h = h;
      this.ratio = ratio;
      this.renderer.setPixelRatio(ratio);
      this.renderer.setSize(w, h, false);
    }
    batch(src, kind) {
      const key = kind + src;
      let b = this.batches.get(key);
      if (!b) {
        this.batches.set(key, (b = new Batch(src, kind)));
        this.root.add(b.mesh);
      }
      return b;
    }
    begin() {
      for (const b of this.batches.values()) b.n = 0;
      this.outline.clear();
    }
    box(b, kind) {
      const alpha = b.alpha == null ? 1 : b.alpha, glow = b.glow || 0;
      for (const f of FACES) {
        const src = b[f.src];
        const flat = src[0] === '#';
        const [r, g, bl] = flat ? toneOf(src) : WHITE;
        const batch = this.batch(flat ? '' : src, kind);
        N[0] = f.n[0]; N[1] = f.n[1]; N[2] = f.n[2];
        if (b.rot) turn(N, b.rot, ZERO);
        for (const i of QUAD) {
          const c = f.at[i];
          P[0] = b.x + c[0] * b.w; P[1] = b.y + c[1] * b.d; P[2] = b.z + c[2] * b.h;
          if (b.rot) turn(P, b.rot, b.rot.at);
          batch.add(P, N, i === 1 || i === 2 ? b[f.u] : 0, i > 1 ? -b[f.v] : 0, r, g, bl, alpha, glow);
        }
      }
    }
    // A Blender model in the cell of the object `o`, posed as RC.POSES describe and moved by the
    // runtime props of the object (aim, reach, level, link). `t` is the moment drawn and `aim`
    // the point the object aims at, if any. Returns the box around the parts it drew, for the
    // pointer.
    model(m, o, pose, kind, t, aim) {
      const batch = this.batch('', kind);
      const alpha = pose.alpha == null ? 1 : pose.alpha;
      const keep = 1 - (pose.dim || 0);
      // The whole model turns about the block centre, then moves to the cell of the object.
      WHOLE.makeTranslation(-0.5, -0.5, -0.5);
      for (const [axis, angle] of pose.turn || []) WHOLE.premultiply(rotation(axis, angle));
      WHOLE.premultiply(SHIFT.makeTranslation(o.px + 0.5, o.py + 0.5, o.pz + 0.5));
      const rig = m.rigged ? motion(m, o, pose, t || 0, aim) : null;
      const stamp = ++placing;
      const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
      for (let i = 0; i < m.parts.length; i++) {
        const part = m.parts[i];
        const how = pose.parts && pose.parts[part.name];
        if (how && how.hide) continue;
        if (part.face && !(o.link && o.link.includes(part.face))) continue;
        if (part.role === 'level' && rig.level < MIN_LEVEL) continue;
        const e = place(m, i, pose, rig, stamp).elements;
        for (const prim of part.prims) {
          const color = prim.tint == null ? null : prim.tint ? pose.tints && pose.tints[prim.tint] : pose.tint;
          const tone = color ? toneOf(color) : prim.tone;
          const r = tone[0] * keep, g = tone[1] * keep, b = tone[2] * keep;
          const glow = prim.glow ? pose.glow || 0 : 0;
          const pos = prim.pos, nrm = prim.nrm;
          for (let i = 0; i < pos.length; i += 3) {
            const x = pos[i], y = pos[i + 1], z = pos[i + 2], nx = nrm[i], ny = nrm[i + 1], nz = nrm[i + 2];
            P[0] = e[0] * x + e[4] * y + e[8] * z + e[12];
            P[1] = e[1] * x + e[5] * y + e[9] * z + e[13];
            P[2] = e[2] * x + e[6] * y + e[10] * z + e[14];
            N[0] = e[0] * nx + e[4] * ny + e[8] * nz;
            N[1] = e[1] * nx + e[5] * ny + e[9] * nz;
            N[2] = e[2] * nx + e[6] * ny + e[10] * nz;
            batch.add(P, N, 0, 0, r, g, b, alpha, glow);
          }
        }
        // The corners of the part's own box, posed the same way, bound what was drawn.
        for (let k = 0; k < 8; k++) {
          const x = k & 1 ? part.hi[0] : part.lo[0], y = k & 2 ? part.hi[1] : part.lo[1], z = k & 4 ? part.hi[2] : part.lo[2];
          for (let a = 0; a < 3; a++) {
            const v = e[a] * x + e[a + 4] * y + e[a + 8] * z + e[a + 12];
            if (v < lo[a]) lo[a] = v;
            if (v > hi[a]) hi[a] = v;
          }
        }
      }
      return { x: lo[0], y: lo[1], z: lo[2], w: hi[0] - lo[0], d: hi[1] - lo[1], h: hi[2] - lo[2], owner: o };
    }
    // A flat strip from p to q, `side` being half its width across. Both windings are sent, so
    // it shows whichever way the camera looks.
    strip(p, q, side, color) {
      const batch = this.batch('', 'over');
      const [r, g, b] = toneOf(color);
      const at = [[p, 1], [q, 1], [q, -1], [p, -1]];
      for (const i of [0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2]) {
        const [c, s] = at[i];
        P[0] = c[0] + s * side[0]; P[1] = c[1] + s * side[1]; P[2] = c[2] + s * side[2];
        batch.add(P, UP, 0, 0, r, g, b, 1, 0);
      }
    }
    end(camera) {
      for (const b of this.batches.values()) b.end();
      this.renderer.render(this.scene, camera);
    }
  }
  let stage = null;

  // ---------------------------------------------------------------- Blender models
  // glb.js hands over every model as base64, because a page opened from disk cannot fetch files.
  // A model becomes plain triangles in block coordinates and goes through the same shader as the
  // boxes, with its material colours as they are. A model is made of parts: every Blender object
  // is one part, named as the object (without Blender's .001 suffix), whose origin is its pivot.
  // RC.POSES[type](o, t) tells how the model answers to the state of its object:
  //   glow   the glow of materials named glow_*;
  //   tint   the colour of the material tint; tints: { key: colour } for materials tint_<key>.
  //          A tinted material may also start with glow_, as glow_tint_<key>;
  //   dim    0..1, darkens the whole model; alpha: 0..1, makes it see-through;
  //   turn   [[axis, angle], ...]: turns of the whole model about the block centre, in order;
  //   parts  { name: { axis, angle, move, hide } }: the part turns about its origin by `angle`
  //          about `axis`, then shifts by `move` ([x, y, z] in blocks); hide leaves it out;
  //   level  the fill level used when the object sets none (see below).
  // Parts named aim_yaw*, aim_pitch*, reach_*, level_* and link_* also answer to the props aim,
  // reach, level and link of the object itself; the header of poses.js describes them.
  const GLB = {};
  RC.POSES = {};
  // Blender (x, y, z) arrives in glTF as (x, z, -y). Scenes use the Blender numbers.
  const FROM_GLTF = new THREE.Matrix4().set(1, 0, 0, 0, 0, 0, -1, 0, 0, 1, 0, 0, 0, 0, 0, 1);
  // One promise per model; it settles once the model is parsed or has failed.
  const parsing = [];
  // Resolves when every model handed over so far is parsed. The loader works through promises
  // alone, so this happens before the next task, and the page is built with the models in place.
  RC.modelsReady = () => Promise.all(parsing);
  RC.glb = (type, base64) => parsing.push(new Promise((done) => parse(type, base64, done)));
  function parse(type, base64, done) {
    const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
    new THREE.GLTFLoader().parse(
      bytes.buffer,
      '',
      (gltf) => {
        const parts = [];
        gltf.scene.updateMatrixWorld(true);
        for (const node of gltf.scene.children) {
          const part = { name: (node.userData.name || '').replace(/\.\d{3}$/, ''), pivot: node.position.clone().applyMatrix4(FROM_GLTF).toArray(), prims: [] };
          const box = new THREE.Box3();
          node.traverse((m) => {
            if (!m.isMesh) return;
            // The models carry no normals, which keeps glb.js small; every face is flat anyway.
            const geo = m.geometry.toNonIndexed().applyMatrix4(m.matrixWorld).applyMatrix4(FROM_GLTF);
            geo.computeVertexNormals();
            geo.computeBoundingBox();
            box.union(geo.boundingBox);
            const name = m.material.name, own = name.replace(/^glow_/, '');
            part.prims.push({
              pos: geo.attributes.position.array,
              nrm: geo.attributes.normal.array,
              tone: m.material.color.clone().convertLinearToSRGB().toArray(),
              glow: name.startsWith('glow'),
              tint: own === 'tint' ? '' : own.startsWith('tint_') ? own.slice(5) : null,
            });
          });
          part.lo = box.min.toArray();
          part.hi = box.max.toArray();
          part.role = roleOf(part.name);
          part.face = part.role === 'link' ? part.name.slice(5) : null;
          part.matrix = new THREE.Matrix4();
          parts.push(part);
        }
        // A part may ride on another one: an aim_pitch part on the aim_yaw part with the same
        // ending, and the parts RC.CARRY lists for this model.
        const carry = (RC.CARRY && RC.CARRY[type]) || {};
        for (const part of parts) {
          const on = carry[part.name] || (part.role === 'pitch' ? 'aim_yaw' + part.name.slice('aim_pitch'.length) : null);
          part.carrier = on ? parts.findIndex((q) => q.name === on) : -1;
        }
        const yaw = parts.map((p, i) => (p.role === 'yaw' ? i : -1)).filter((i) => i >= 0);
        GLB[type] = { parts, yaw, rigged: parts.some((p) => p.role && p.role !== 'link') };
        done();
      },
      // Rethrown outside the loader's promise, so the page's error line shows it.
      (e) => {
        setTimeout(() => { throw e; });
        done();
      }
    );
  }

  // ---------------------------------------------------------------- runtime props of models
  // What a part does by its name: aim_yaw* turns to the aim about the vertical, aim_pitch* tilts
  // to it, reach_* slides out, level_* fills up from its base, link_<face> shows when linked.
  const ROLES = [['aim_yaw', 'yaw'], ['aim_pitch', 'pitch'], ['reach_', 'reach'], ['level_', 'level'], ['link_', 'link']];
  const roleOf = (name) => (ROLES.find(([prefix]) => name.startsWith(prefix)) || [])[1] || null;
  // How far a reach_ part slides out at reach 1, along x of its model (or of the part it rides on).
  const REACH = 0.5;
  // The time constant, in seconds, with which aim, reach and level follow a new target.
  const EASE = 0.35;
  // A barrel tilts no further down or up than this, in radians.
  const PITCH_DOWN = -0.6, PITCH_UP = 1.2;
  // A level part below this share is not drawn at all.
  const MIN_LEVEL = 0.005;
  const FACE_DIR = { 'x+': [1, 0, 0], 'x-': [-1, 0, 0], 'y+': [0, 1, 0], 'y-': [0, -1, 0], 'z+': [0, 0, 1], 'z-': [0, 0, -1] };
  const STRETCH = new THREE.Matrix4(), INVERSE = new THREE.Matrix4(), AIM = new THREE.Vector3();
  const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
  const wrapAngle = (a) => a - 2 * Math.PI * Math.round(a / (2 * Math.PI));

  // The point in scene numbers an object aims at: its aim prop as a point, as a side of its block
  // ('x+', ...; four blocks out), or as the id of another object of the scene (its centre).
  function aimPoint(o, scene) {
    const a = o.aim;
    if (!a) return null;
    if (Array.isArray(a)) return a;
    const d = FACE_DIR[a];
    if (d) return [o.px + 0.5 + 4 * d[0], o.py + 0.5 + 4 * d[1], o.pz + 0.5 + 4 * d[2]];
    const q = scene && scene.objs.get(a);
    return q && q !== o ? [q.px + 0.5, q.py + 0.5, q.pz + 0.5] : null;
  }

  // Yaw, pitch, reach and level of a rigged model at the moment `t`. Each glides towards its
  // target; the state lives on the object as _rig and starts at the target the first time.
  // WHOLE must already hold the turn of the whole model.
  function motion(m, o, pose, t, aim) {
    const level = o.level != null ? o.level : pose.level != null ? pose.level : 1;
    const goal = { yaw: 0, pitch: 0, reach: clamp(o.reach || 0, 0, 1), level: clamp(level, 0, 1) };
    if (aim && m.yaw.length) {
      // The aim in model numbers, seen from the first aim_yaw part the pose shows.
      const shown = (i) => !(pose.parts && pose.parts[m.parts[i].name] && pose.parts[m.parts[i].name].hide);
      const head = m.yaw.find(shown);
      const y = head == null ? m.yaw[0] : head;
      AIM.fromArray(aim).applyMatrix4(INVERSE.copy(WHOLE).invert());
      const p = m.parts[y].pivot;
      const barrel = m.parts.find((q) => q.role === 'pitch' && q.carrier === y);
      const dx = AIM.x - p[0], dy = AIM.y - p[1], flat = Math.hypot(dx, dy);
      if (flat > EPS) goal.yaw = Math.atan2(dy, dx);
      goal.pitch = clamp(Math.atan2(AIM.z - (barrel ? barrel.pivot[2] : p[2]), flat), PITCH_DOWN, PITCH_UP);
    }
    const s = o._rig || (o._rig = Object.assign({ t }, goal));
    const k = 1 - Math.exp(-Math.max(0, t - s.t) / (o.ease > 0 ? o.ease : EASE));
    s.t = t;
    s.yaw += wrapAngle(goal.yaw - s.yaw) * k;
    s.pitch += (goal.pitch - s.pitch) * k;
    s.reach += (goal.reach - s.reach) * k;
    s.level += (goal.level - s.level) * k;
    return s;
  }

  // The matrix that takes part i of the model from model numbers to scene numbers: the level
  // stretch, the pose turn about its origin, the pose shift, the aim turn, the reach slide, then the
  // part it rides on or the whole model. `stamp` marks the parts already placed in this drawing.
  function place(m, i, pose, rig, stamp) {
    const part = m.parts[i], M = part.matrix;
    if (part.stamp === stamp) return M;
    part.stamp = stamp;
    const how = pose.parts && pose.parts[part.name];
    const p = part.pivot, mv = (how && how.move) || ZERO;
    if (part.role === 'level') {
      // Stretched upright about the bottom of the part, then taken to its origin as usual.
      const base = part.lo[2];
      M.makeTranslation(0, 0, -base).premultiply(STRETCH.makeScale(1, 1, rig.level)).premultiply(SHIFT.makeTranslation(-p[0], -p[1], base - p[2]));
    } else M.makeTranslation(-p[0], -p[1], -p[2]);
    if (how && how.axis) M.premultiply(rotation(how.axis, how.angle));
    M.premultiply(SHIFT.makeTranslation(mv[0], mv[1], mv[2]));
    if (part.role === 'yaw') M.premultiply(rotation('z', rig.yaw));
    if (part.role === 'pitch') M.premultiply(rotation('y', -rig.pitch));
    M.premultiply(SHIFT.makeTranslation(p[0] + (part.role === 'reach' ? rig.reach * REACH : 0), p[1], p[2]));
    return M.premultiply(part.carrier >= 0 ? place(m, part.carrier, pose, rig, stamp) : WHOLE);
  }
  let placing = 0;

  // Draws the objects of a scene at the moment `t`: boxes, and the Blender model in place of the
  // boxes of every type that has one. `look` is where the camera looks, in scene numbers. Returns
  // everything the pointer can hit: boxes, and one box around every model.
  function drawObjects(st, scene, t, look) {
    const depth = (b) => (b.x + b.w / 2) * look[0] + (b.y + b.d / 2) * look[1] + (b.z + b.h / 2) * look[2];
    const solids = [];
    const clear = [];
    for (const b of scene.build(t)) {
      if (GLB[b.owner.type]) continue;
      solids.push(b);
      if (b.alpha == null || b.alpha >= 1) st.box(b, 'solid');
      else clear.push(b);
    }
    for (const o of scene.objs.values()) {
      const m = GLB[o.type];
      if (!m) continue;
      const pose = RC.POSES[o.type] ? RC.POSES[o.type](o, t) : {};
      const aim = aimPoint(o, scene);
      if (pose.alpha < 1) clear.push({ x: o.px, y: o.py, z: o.pz, w: 1, d: 1, h: 1, model: m, pose, aim, owner: o });
      else solids.push(st.model(m, o, pose, 'solid', t, aim));
    }
    // See-through boxes and models go from the far ones to the near ones, so each blends over
    // what is behind it.
    for (const b of clear.sort((p, q) => depth(q) - depth(p))) {
      if (b.model) solids.push(st.model(b.model, b.owner, b.pose, 'clear', t, b.aim));
      else st.box(b, 'clear');
    }
    return solids;
  }

  // ---------------------------------------------------------------- block icons
  // Icons are drawn by one offscreen stage of their own, with the scene shader and the default
  // scene view, and copied into plain 2D canvases. A block with a Blender model is shown at work.
  const WORKING = { on: true, busy: true };
  // The moment an icon shows: moving parts stand as modelled at 0; the crafter has all lamps lit.
  const ICON_TIME = { crafter: 2.25 };
  // Room around the block, as a share of the icon, so smoothed edges are not cut.
  const ICON_MARGIN = 0.03;
  const ICON_AWAY = 100;
  const iconLook = [-1, -1, -1].map((v) => v / Math.sqrt(3));
  const iconCache = new Map();
  let iconStage = null, iconCam = null;

  function renderIcon(type, props, px) {
    if (!iconStage) {
      iconStage = new Stage();
      iconStage.tint.visible = false;
      iconStage.renderer.setPixelRatio(1);
      iconCam = new THREE.OrthographicCamera(-1, 1, 1, -1, 1, 4 * ICON_AWAY);
      iconCam.position.set(ICON_AWAY, ICON_AWAY, ICON_AWAY);
      iconCam.lookAt(0, 0, 0);
      iconCam.updateMatrixWorld();
    }
    const st = iconStage, cam = iconCam;
    const scene = new Scene().put('o', type, 0, 0, 0, GLB[type] ? Object.assign({}, WORKING, props) : props);
    st.begin();
    drawObjects(st, scene, ICON_TIME[type] || 0, iconLook);
    // Frame everything drawn: its bounds across and up the picture, made square.
    const e = cam.matrixWorldInverse.elements;
    let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
    for (const b of st.batches.values())
      for (let i = 0, d = b.data; i < b.n * VERT; i += VERT) {
        // Scene numbers (x, y, z) are (x, z, y) for three.js.
        const x = d[i], y = d[i + 2], z = d[i + 1];
        const u = e[0] * x + e[4] * y + e[8] * z, v = e[1] * x + e[5] * y + e[9] * z;
        if (u < x0) x0 = u;
        if (u > x1) x1 = u;
        if (v < y0) y0 = v;
        if (v > y1) y1 = v;
      }
    const half = (Math.max(x1 - x0, y1 - y0) / 2) * (1 + 2 * ICON_MARGIN);
    const cx = (x0 + x1) / 2 + e[12], cy = (y0 + y1) / 2 + e[13];
    cam.left = cx - half;
    cam.right = cx + half;
    cam.bottom = cy - half;
    cam.top = cy + half;
    cam.updateProjectionMatrix();
    if (st.size !== px) {
      st.size = px;
      st.renderer.setSize(px, px, false);
    }
    st.end(cam);
    const c = document.createElement('canvas');
    c.width = c.height = px;
    c.getContext('2d').drawImage(st.canvas, 0, 0);
    return c;
  }

  // A block icon `size` CSS pixels wide, sharp at the pixel density of the screen.
  RC.thumb = (type, props, size) => {
    const px = Math.round(size * (window.devicePixelRatio || 1));
    const key = type + JSON.stringify(props || {}) + px;
    let src = iconCache.get(key);
    if (!src) iconCache.set(key, (src = renderIcon(type, props, px)));
    const c = document.createElement('canvas');
    c.width = c.height = px;
    c.getContext('2d').drawImage(src, 0, 0);
    return c;
  };

  // Distance along the ray to the box, or Infinity when the ray misses it.
  function hitBox(from, dir, b) {
    const lo = [b.x, b.y, b.z], hi = [b.x + b.w, b.y + b.d, b.z + b.h];
    let near = -Infinity, far = Infinity;
    for (let i = 0; i < 3; i++) {
      if (Math.abs(dir[i]) < EPS) {
        if (from[i] < lo[i] || from[i] > hi[i]) return Infinity;
        continue;
      }
      const a = (lo[i] - from[i]) / dir[i], c = (hi[i] - from[i]) / dir[i];
      near = Math.max(near, Math.min(a, c));
      far = Math.min(far, Math.max(a, c));
    }
    return near <= far ? near : Infinity;
  }

  // ---------------------------------------------------------------- player
  // The widest the default view zooms in: about eight blocks across the picture.
  const MIN_HALF_WIDTH = 4.1;
  const VIEW_PAD = 0.35;
  // Room kept above a label's anchor point, as a share of the picture height.
  const LABEL_ROOM = 0.1;
  const CAMERA_AWAY = 100;
  // A pointer that moved further than this between press and release turned the camera.
  const DRAG_PX = 4;
  const DOT = 0.13, DOT_RIM = 0.025;
  const BEAM_WIDTH = 0.07, DASH = 0.19, DASH_GAP = 0.13, DASH_SPEED = 0.9;
  // Height above its feet at which a beam meets the object it follows.
  const BEAM_BODY = 1.1;
  // A step stays on screen long enough to read its caption: about 14 characters a second.
  const stepTime = (st) => Math.max(st.dur || 5, 3 + st.text.length / 14);
  const CHIP = { i: ['#ffa53c', 'предметы'], f: ['#4c9bf5', 'жидкость'], e: ['#ffe14a', 'энергия'] };
  const chips = (list) =>
    list.map(([k, what]) => '<span class="chip"><i style="background:' + CHIP[k][0] + '"></i>' + CHIP[k][1] + (what ? ': ' + what : '') + '</span>').join('');
  const cube = (x, y, z, size, top, side) => ({ x: x - size / 2, y: y - size / 2, z: z - size / 2, w: size, d: size, h: size, top, left: side, right: side });

  class Player {
    // def: { steps: [{ text, dur, run(scene) }] }
    constructor(root, def) {
      this.def = def;
      this.root = root;
      root.innerHTML =
        '<div class="stage"><div class="view"><div class="badges"></div><div class="tip" hidden></div></div></div>' +
        '<div class="legend-row"><span><i style="background:#ffa53c"></i>предметы</span><span><i style="background:#4c9bf5"></i>жидкость</span><span><i style="background:#ffe14a"></i>энергия</span><span class="hint">Наведи на любой объект сцены, чтобы узнать, что это. Клик открывает его страницу. Сцену можно вращать мышью или пальцем, ведя его вбок. Приближают Ctrl и колесо или два пальца.</span></div>' +
        '<div class="onstage"></div><div class="caption" aria-live="polite"></div>' +
        '<div class="controls"><button data-a="prev" title="Предыдущий шаг" aria-label="Предыдущий шаг">◀</button><button data-a="play" title="Пауза или пуск" aria-label="Пауза или пуск">❚❚</button>' +
        '<button data-a="next" title="Следующий шаг" aria-label="Следующий шаг">▶</button><button data-a="again" title="С начала" aria-label="С начала">↺</button><span class="dots"></span><span class="count">шаг <span class="n"></span> из ' + def.steps.length + '</span>' +
        '<button data-a="link" class="reset" title="Скопировать ссылку, которая открывает сцену на этом шаге">Ссылка на шаг</button>' +
        '<button data-a="view" class="reset" title="Вернуть камеру в исходное положение">Вид по умолчанию</button>' +
        '<label class="tagsw"><input type="checkbox" checked> названия блоков</label></div>';
      this.stage = stage || (stage = new Stage());
      this.view = root.querySelector('.view');
      this.view.prepend(this.stage.canvas);
      this.badgeBox = root.querySelector('.badges');
      this.caption = root.querySelector('.caption');
      this.dots = root.querySelector('.dots');
      this.count = root.querySelector('.count');
      this.onstage = root.querySelector('.onstage');
      this.tip = root.querySelector('.tip');
      this.showTags = true;
      this.hover = null;
      this.solids = [];
      root.querySelector('.tagsw input').addEventListener('change', (e) => {
        this.showTags = e.target.checked;
      });
      // People who ask for less motion get a paused scene and step through it themselves.
      this.playing = !(window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches);
      if (!this.playing) root.querySelector('[data-a=play]').textContent = '►';
      this.fit();
      this.bindHover();
      root.querySelector('.controls').addEventListener('click', (e) => {
        const a = e.target.dataset.a;
        if (a === 'prev') this.go(Math.max(0, this.index - 1));
        if (a === 'next') this.go(Math.min(def.steps.length - 1, this.index + 1));
        if (a === 'again') this.go(0);
        if (a === 'view') this.controls.reset();
        // The address #crusher/3 opens the page on the third step (see show() in app.js).
        if (a === 'link') Kit.copy(location.href.split('#')[0] + '#' + location.hash.slice(1).split('/')[0] + '/' + (this.index + 1), 'Ссылка на шаг ' + (this.index + 1) + ' скопирована');
        if (a === 'play') {
          this.playing = !this.playing;
          e.target.textContent = this.playing ? '❚❚' : '►';
          this.last = performance.now();
        }
        if (e.target.dataset.i) this.go(+e.target.dataset.i);
      });
      this.dots.innerHTML = def.steps.map((s, i) => '<button class="dot" data-i="' + i + '" aria-label="Шаг ' + (i + 1) + '"></button>').join('');
      this.go(0);
      this.last = performance.now();
      this.alive = true;
      const loop = (now) => {
        if (!this.alive || !root.isConnected) return;
        // Time stands still while the pointer inspects an object, so moving things can be read.
        if (this.playing && !this.hover) {
          this.time += Math.min(0.1, (now - this.last) / 1000);
          const hold = stepTime(def.steps[this.index]);
          if (this.time - this.stepStart > hold) {
            if (this.index < def.steps.length - 1) this.go(this.index + 1);
            else if (this.time - this.stepStart > hold + 3) this.go(0);
          }
        }
        this.last = now;
        this.draw();
        requestAnimationFrame(loop);
      };
      requestAnimationFrame(loop);
    }
    stop() {
      this.alive = false;
      this.controls.dispose();
    }
    // Replays the steps on a scratch scene to find how much room the whole scene needs, then sets
    // the default view: an orthographic camera that looks from +x, +y, +z and frames all of it.
    fit() {
      const cam = (this.camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 1, 4 * CAMERA_AWAY));
      cam.position.set(1, 1, 1);
      cam.lookAt(0, 0, 0);
      cam.updateMatrixWorld();
      // Bounds are measured in camera space: x across the picture, y up, z towards the viewer.
      const box = new THREE.Box3();
      const v = new THREE.Vector3();
      const add = (p) => box.expandByPoint(v.set(p[0], p[2], p[1]).applyMatrix4(cam.matrixWorldInverse));
      const cell = (p) => corners({ x: p[0], y: p[1], z: p[2], w: 1, d: 1, h: 1.2 }).forEach(add);
      const labels = [];
      const s = new Scene();
      for (const step of this.def.steps) {
        step.run(s);
        for (const b of s.build(0)) {
          if (!b.rot) corners(b).forEach(add);
          // A turning box needs room for the whole turn.
          else for (let k = 0; k < 8; k++) corners(Object.assign({}, b, { rot: Object.assign({}, b.rot, { angle: (k * Math.PI) / 4 }) })).forEach(add);
        }
        for (const o of s.objs.values()) for (const p of o.path || []) cell(p);
        for (const f of s.flows) for (const p of f.pts) cell(p);
        for (const b of s.badges) labels.push([b.x + 0.5, b.y + 0.5, b.z + 2.15]);
        for (const tg of s.tags) {
          const o = s.objs.get(tg.obj);
          if (o) labels.push([o.x + 0.5, o.y + 0.5, o.z + (tg.lift || 1.45)]);
        }
      }
      const size = new THREE.Vector3();
      const halfWidth = () => Math.max(MIN_HALF_WIDTH, box.getSize(size).x / 2 + VIEW_PAD, ((size.y / 2 + VIEW_PAD) * LW) / LH);
      // Labels stand above their anchor points; one unit of z is about 0.82 of a unit up the picture.
      const lift = (LABEL_ROOM * 2 * halfWidth() * LH) / LW / 0.82;
      for (const p of labels) add([p[0], p[1], p[2] + lift]);
      cam.right = halfWidth();
      cam.left = -cam.right;
      cam.top = (cam.right * LH) / LW;
      cam.bottom = -cam.top;
      cam.updateProjectionMatrix();
      const target = box.getCenter(new THREE.Vector3()).applyMatrix4(cam.matrixWorld);
      cam.position.copy(target).addScalar(CAMERA_AWAY / Math.sqrt(3));
      this.controls = new THREE.OrbitControls(cam, this.view);
      this.controls.target.copy(target);
      this.controls.minZoom = 0.4;
      this.controls.maxZoom = 5;
      // The camera stays above the ground.
      this.controls.maxPolarAngle = Math.PI / 2;
      // A plain wheel scrolls the page; the wheel zooms the scene only while Ctrl is held.
      this.controls.enableZoom = false;
      this.view.addEventListener('wheel', (e) => { this.controls.enableZoom = e.ctrlKey; }, { capture: true, passive: true });
      // On a touch screen an upright swipe scrolls the page, a sideways one turns the scene and
      // two fingers zoom it.
      this.view.style.touchAction = 'pan-y';
      this.view.addEventListener('pointerdown', (e) => { if (e.pointerType === 'touch') this.controls.enableZoom = true; }, { capture: true });
      this.controls.update();
      this.controls.saveState();
    }
    go(i) {
      // Scene state is cumulative, so a jump rebuilds it from the first step.
      const old = this.scene;
      this.scene = new Scene();
      this.time = 0;
      for (let j = 0; j <= i; j++) {
        this.scene.now = this.time;
        this.def.steps[j].run(this.scene);
        if (j < i) this.time += stepTime(this.def.steps[j]);
      }
      // Aim, reach and level glide on from where the scene on screen left them.
      if (old)
        for (const o of this.scene.objs.values()) {
          const was = old.objs.get(o.id);
          if (was && was._rig && was.type === o.type) o._rig = Object.assign({}, was._rig, { t: this.time });
        }
      this.index = i;
      this.stepStart = this.time;
      this.caption.textContent = this.def.steps[i].text;
      Kit.flow(this.count.querySelector('.n'), i + 1);
      this.onstage.innerHTML = this.scene.tags.length
        ? 'На сцене: ' + this.scene.tags.map((tg) => {
            const info = RC.info(this.scene.objs.get(tg.obj));
            return info && info.entry ? '<a href="#' + info.entry + '">' + tg.text + '</a>' : tg.text;
          }).join(' · ')
        : '';
      this.setHover(null);
      [...this.dots.children].forEach((d, j) => d.classList.toggle('on', j === i));
    }
    draw() {
      const st = this.stage, cam = this.camera;
      const w = this.view.clientWidth, h = Math.round((w * LH) / LW);
      if (!w) return;
      st.resize(w, h);
      cam.updateMatrixWorld();
      // Where the camera looks, in scene numbers.
      const v = cam.getWorldDirection(new THREE.Vector3());
      const look = [v.x, v.z, v.y];
      st.begin();
      this.solids = drawObjects(st, this.scene, this.time, look);
      this.drawOverlays(look);
      st.end(cam);
      this.drawLabels(w, h);
    }
    // Night veil, beams, flow dots and the outline of the hovered object.
    drawOverlays(look) {
      const st = this.stage, scene = this.scene, t = this.time;
      const rgba = scene.tint ? scene.tint.match(/[\d.]+/g).map(Number) : null;
      st.tint.visible = !!rgba;
      if (rgba) st.tint.material.uniforms.color.value.set(rgba[0] / 255, rgba[1] / 255, rgba[2] / 255, rgba[3]);

      // A beam is a row of dashes that run from its start to its end. Its end may be the id of an
      // object: the beam then follows the object, to the middle of its body.
      const endOf = (b) => {
        if (typeof b !== 'string') return b;
        const o = scene.objs.get(b);
        return o && [o.px + 0.5, o.py + 0.5, o.pz + BEAM_BODY];
      };
      for (const beam of scene.beams) {
        const bm = { a: beam.a, b: endOf(beam.b), color: beam.color };
        if (!bm.b) continue;
        const d = [bm.b[0] - bm.a[0], bm.b[1] - bm.a[1], bm.b[2] - bm.a[2]];
        const len = Math.hypot(d[0], d[1], d[2]);
        if (!len) continue;
        const across = new THREE.Vector3(d[0], d[1], d[2]).cross(new THREE.Vector3(look[0], look[1], look[2])).setLength(BEAM_WIDTH / 2).toArray();
        const point = (s) => [bm.a[0] + (d[0] * s) / len, bm.a[1] + (d[1] * s) / len, bm.a[2] + (d[2] * s) / len];
        const period = DASH + DASH_GAP;
        for (let s = ((t * DASH_SPEED) % period) - period; s < len; s += period)
          if (s + DASH > 0) st.strip(point(Math.max(0, s)), point(Math.min(len, s + DASH)), across, bm.color);
      }
      // A flow dot is a small cube with a dark rim, in the middle of the cell it passes.
      for (const f of scene.flows) {
        const len = pathLength(f.pts);
        for (let i = 0; i < f.n; i++) {
          const p = pointOnPath(f.pts, (t * f.speed + (i * len) / f.n) % len);
          st.box(cube(p[0] + 0.5, p[1] + 0.5, p[2] + 0.5, DOT + 2 * DOT_RIM, DARK, DARK), 'over');
          st.box(cube(p[0] + 0.5, p[1] + 0.5, p[2] + 0.5, DOT, RC.shade(f.color, 0.5), f.color), 'over');
        }
      }

      if (this.hover)
        for (const b of this.solids) {
          if (b.owner !== this.hover) continue;
          const line = new THREE.LineSegments(st.edges, st.line);
          const c = [b.x + b.w / 2, b.y + b.d / 2, b.z + b.h / 2];
          if (b.rot) {
            turn(c, b.rot, b.rot.at);
            line.rotation[b.rot.axis] = b.rot.angle;
          }
          line.position.fromArray(c);
          // A little larger than the box, so the edges do not sink into its faces.
          line.scale.set(b.w + 0.02, b.d + 0.02, b.h + 0.02);
          st.outline.add(line);
        }
    }
    // Labels are HTML over the canvas, placed where their scene points land on screen.
    drawLabels(w, h) {
      const scene = this.scene, cam = this.camera;
      const v = new THREE.Vector3();
      const at = (x, y, z, cls, text) => {
        v.set(x + 0.5, z, y + 0.5).project(cam);
        return '<span class="badge ' + cls + '" style="left:' + Math.round(((v.x + 1) / 2) * w) + 'px;top:' + Math.round(((1 - v.y) / 2) * h) + 'px">' + text + '</span>';
      };
      let html = '';
      if (this.showTags)
        for (const tg of scene.tags) {
          const o = scene.objs.get(tg.obj);
          if (o) html += at(o.px, o.py, o.pz + (tg.lift || 1.45), 'tag', tg.text);
        }
      for (const b of scene.badges) html += at(b.x, b.y, b.z + 2.15, b.kind, b.text);
      if (html !== this.badgeHtml) this.badgeBox.innerHTML = this.badgeHtml = html;
    }
    // The object under a point of the view, given in CSS pixels: the nearest one the ray meets.
    pick(x, y) {
      const cam = this.camera;
      const p = new THREE.Vector3((x / this.view.clientWidth) * 2 - 1, 1 - (y / this.view.clientHeight) * 2, -1).unproject(cam);
      const v = cam.getWorldDirection(new THREE.Vector3());
      const from = [p.x, p.z, p.y], dir = [v.x, v.z, v.y];
      let best = null, near = Infinity;
      for (const b of this.solids) {
        // A turned box is hit by turning the ray back instead.
        const back = b.rot && { axis: b.rot.axis, angle: -b.rot.angle };
        const d = back ? hitBox(turn(from.slice(), back, b.rot.at), turn(dir.slice(), back, ZERO), b) : hitBox(from, dir, b);
        if (d < near) {
          near = d;
          best = b.owner;
        }
      }
      return best;
    }
    setHover(o, cx, cy) {
      this.hover = o;
      const info = o && RC.info ? RC.info(o) : null;
      this.view.style.cursor = info && info.entry ? 'pointer' : 'default';
      if (!info) {
        this.hover = null;
        this.tip.hidden = true;
        return;
      }
      this.tip.innerHTML =
        '<b>' + info.name + '</b><p>' + info.does + '</p>' +
        (info.takes && info.takes.length ? '<div class="io"><em>принимает</em>' + chips(info.takes) + '</div>' : '') +
        (info.gives && info.gives.length ? '<div class="io"><em>отдаёт</em>' + chips(info.gives) + '</div>' : '') +
        (info.entry ? '<div class="go">клик: открыть страницу</div>' : '');
      this.tip.hidden = false;
      const w = this.tip.offsetWidth, h = this.tip.offsetHeight;
      this.tip.style.left = Math.max(4, Math.min(this.view.clientWidth - w - 4, cx + 14)) + 'px';
      this.tip.style.top = Math.max(4, Math.min(this.view.clientHeight - h - 4, cy + 14)) + 'px';
    }
    bindHover() {
      const view = this.view;
      const where = (e) => {
        const r = view.getBoundingClientRect();
        return [e.clientX - r.left, e.clientY - r.top];
      };
      let down = null, dragged = false;
      view.addEventListener('pointerdown', (e) => {
        down = where(e);
        dragged = false;
      });
      view.addEventListener('pointermove', (e) => {
        const [x, y] = where(e);
        if (!e.buttons) return this.setHover(this.pick(x, y), x, y);
        // A pressed pointer turns the camera; the card would only be in the way.
        if (down && Math.hypot(x - down[0], y - down[1]) > DRAG_PX) {
          dragged = true;
          this.setHover(null);
        }
      });
      view.addEventListener('pointerleave', (e) => {
        if (e.pointerType === 'mouse') this.setHover(null);
      });
      view.addEventListener('click', (e) => {
        if (dragged) return;
        const [x, y] = where(e);
        const o = this.pick(x, y);
        // A touch shows the card first; the second tap on the same object opens its page.
        const shown = o && this.tipFor === o && !this.tip.hidden;
        this.setHover(o, x, y);
        const info = o && RC.info ? RC.info(o) : null;
        if ((shown || e.pointerType === 'mouse') && info && info.entry) location.hash = info.entry;
        this.tipFor = o;
      });
    }
  }
  RC.Player = Player;
})();
