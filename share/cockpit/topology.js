// The capability constellation: the registry graph in three dimensions.
//
// What a third dimension is for here: the registry graph is a layered thing — modules
// compose capabilities, capabilities are declared in files and project onto MCP, HTTP and
// the command line — and a flat drawing has to choose between showing the layers and
// showing the fan-out. Putting each layer on its own plane shows both at once: how wide
// each projection is, and which module feeds it. That is the whole reason this view
// exists, and it is why nothing spins for its own sake.
//
// Strictly optional. The library is vendored, loaded only here, and never required: with
// no WebGL, no Three.js and no JavaScript at all, the same facts are on
// /cockpit/graphs/registry as text, and this page says so.

import { api, vendorModule, palette, explainMissing, motionAllowed, whileVisible } from './cockpit.js';

const frame = document.querySelector('[data-mj-topology]');
if (frame) {
  draw(frame).catch((error) => explainMissing(frame, error));
}

/** Which plane a node kind sits on, and how far apart the planes are. */
const PLANES = { module: 0, capability: 26, source: -26, projection: 52 };

async function draw(frame) {
  if (!window.WebGLRenderingContext) {
    throw new Error('this browser has no WebGL');
  }
  const [THREE, answer] = await Promise.all([
    vendorModule('three.module.min.js'),
    api(frame.dataset.mjTopologySrc),
  ]);
  if (!answer.ok) throw new Error('the graph endpoint answered ' + answer.status);
  const graph = answer.body;
  const colours = palette();

  const width = frame.clientWidth || 800;
  const height = frame.clientHeight || 560;

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(45, width / height, 1, 2000);
  camera.position.set(120, 90, 190);
  camera.lookAt(0, 14, 0);

  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setSize(width, height, false);
  frame.textContent = '';
  frame.appendChild(renderer.domElement);
  renderer.domElement.style.display = 'block';

  // one ring of positions per plane, in the graph's own node order, so the same repository
  // always produces the same picture
  const byPlane = new Map();
  for (const node of graph.nodes) {
    const plane = PLANES[node.kind] === undefined ? 0 : PLANES[node.kind];
    if (!byPlane.has(plane)) byPlane.set(plane, []);
    byPlane.get(plane).push(node);
  }
  const positions = new Map();
  for (const [y, nodes] of byPlane) {
    const radius = 20 + nodes.length * 2.2;
    nodes.forEach((node, i) => {
      const angle = (i / nodes.length) * Math.PI * 2;
      positions.set(node.id, new THREE.Vector3(Math.cos(angle) * radius, y, Math.sin(angle) * radius));
    });
  }

  const kinds = Object.keys(graph.node_kinds);
  const colourOf = (kind) => {
    const at = Math.max(kinds.indexOf(kind), 0);
    return new THREE.Color().setHSL(((at / Math.max(kinds.length, 1)) * 0.8 + 0.55) % 1, 0.5, 0.55);
  };

  const sphere = new THREE.SphereGeometry(1.6, 12, 12);
  const group = new THREE.Group();
  for (const node of graph.nodes) {
    const mesh = new THREE.Mesh(
      sphere,
      new THREE.MeshBasicMaterial({ color: colourOf(node.kind) }),
    );
    mesh.position.copy(positions.get(node.id));
    mesh.scale.setScalar(node.kind === 'projection' ? 2.2 : 1);
    group.add(mesh);
  }

  const lines = [];
  for (const edge of graph.edges) {
    const from = positions.get(edge.source);
    const to = positions.get(edge.target);
    if (!from || !to) continue;
    lines.push(from.x, from.y, from.z, to.x, to.y, to.z);
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(lines, 3));
  group.add(
    new THREE.LineSegments(
      geometry,
      new THREE.LineBasicMaterial({
        color: new THREE.Color(colours.dark ? 0x64748b : 0x94a3b8),
        transparent: true,
        opacity: 0.5,
      }),
    ),
  );
  scene.add(group);

  const render = () => renderer.render(scene, camera);
  render();

  // pointer drag turns the constellation; that is the only interaction, and it is the one
  // a person actually wants from a 3D view
  let dragging = false;
  let last = { x: 0, y: 0 };
  renderer.domElement.addEventListener('pointerdown', (event) => {
    dragging = true;
    last = { x: event.clientX, y: event.clientY };
    renderer.domElement.setPointerCapture(event.pointerId);
  });
  renderer.domElement.addEventListener('pointerup', () => {
    dragging = false;
  });
  renderer.domElement.addEventListener('pointermove', (event) => {
    if (!dragging) return;
    group.rotation.y += (event.clientX - last.x) * 0.005;
    group.rotation.x += (event.clientY - last.y) * 0.005;
    last = { x: event.clientX, y: event.clientY };
    render();
  });

  window.addEventListener('resize', () => {
    const w = frame.clientWidth || width;
    const h = frame.clientHeight || height;
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    renderer.setSize(w, h, false);
    render();
  });

  // a slow turn, only when motion is wanted, only while the frame is on screen and the tab
  // is visible; a reader who asked for reduced motion gets the still picture and the drag
  if (motionAllowed()) {
    whileVisible(frame, () => {
      if (dragging) return;
      group.rotation.y += 0.0012;
      render();
    });
  }

  // WebGL contexts are a scarce resource: give it back when the page goes away
  window.addEventListener('pagehide', () => {
    geometry.dispose();
    sphere.dispose();
    renderer.dispose();
  });
}
