import * as THREE from 'three';
import { Line2 } from 'three/examples/jsm/lines/Line2.js';
import { LineGeometry } from 'three/examples/jsm/lines/LineGeometry.js';
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js';
import { getScene, getCamera, getControls, getRenderer } from './scene';
import { setPickedLineNum } from '../../lib/store';
import type { GCodeFileInfo, ArcData } from '../../lib/types';

let rapidLines: THREE.LineSegments | null = null;
let feedLines: THREE.LineSegments | null = null;

const RAPID_COLOR = [0.3, 0.3, 0.5]; // dim blue-gray
const FEED_COLOR = [0.2, 0.8, 0.4];  // green
const DONE_COLOR = [0.35, 0.35, 0.35]; // gray for completed lines

// Mapping from G-code line index to vertex range in feed/rapid geometry
interface VertexMapping {
  target: 'rapid' | 'feed';
  vertexStart: number; // index into the color buffer (vertex index)
  vertexCount: number;
}

let vertexMap: (VertexMapping | null)[] = [];
let lastProgressLine = -1;

// Fat-line overlay for highlighted/picked line
let highlightLine: Line2 | null = null;

/**
 * Interpolate a G2/G3 arc into line segment positions.
 */
function interpolateArc(
  sx: number, sy: number, sz: number,
  ex: number, ey: number, ez: number,
  arc: ArcData,
  clockwise: boolean,
): number[] {
  let sp: number, ss: number, sn: number;
  let ep: number, es: number, en: number;
  let co1: number, co2: number;

  switch (arc.plane) {
    case 18:
      sp = sz; ss = sx; sn = sy;
      ep = ez; es = ex; en = ey;
      co1 = arc.k; co2 = arc.i;
      break;
    case 19:
      sp = sy; ss = sz; sn = sx;
      ep = ey; es = ez; en = ex;
      co1 = arc.j; co2 = arc.k;
      break;
    default:
      sp = sx; ss = sy; sn = sz;
      ep = ex; es = ey; en = ez;
      co1 = arc.i; co2 = arc.j;
      break;
  }

  const cp = sp + co1;
  const cs = ss + co2;
  const r = Math.sqrt(co1 * co1 + co2 * co2);
  if (r < 1e-6) {
    return [sx, sy, sz, ex, ey, ez];
  }

  const a0 = Math.atan2(ss - cs, sp - cp);
  const a1 = Math.atan2(es - cs, ep - cp);

  let sweep = a1 - a0;
  if (clockwise) {
    if (sweep > -1e-10) sweep -= 2 * Math.PI;
  } else {
    if (sweep < 1e-10) sweep += 2 * Math.PI;
  }

  const n = Math.max(8, Math.ceil(Math.abs(sweep) / (5 * Math.PI / 180)));
  const out: number[] = [];
  let px = sx, py = sy, pz = sz;

  for (let seg = 1; seg <= n; seg++) {
    const t = seg / n;
    const angle = a0 + sweep * t;
    const nv = sn + (en - sn) * t;
    const pv = cp + r * Math.cos(angle);
    const sv = cs + r * Math.sin(angle);

    let x: number, y: number, z: number;
    switch (arc.plane) {
      case 18: z = pv; x = sv; y = nv; break;
      case 19: y = pv; z = sv; x = nv; break;
      default: x = pv; y = sv; z = nv; break;
    }

    out.push(px, py, pz, x, y, z);
    px = x; py = y; pz = z;
  }

  return out;
}

export function updateToolpath(file: GCodeFileInfo): void {
  const scene = getScene();
  if (!scene) return;

  clearToolpath();

  const rapidPositions: number[] = [];
  const feedPositions: number[] = [];
  vertexMap = [];
  lastProgressLine = -1;

  let prevX = 0, prevY = 0, prevZ = 0;

  for (let i = 0; i < file.lines.length; i++) {
    const line = file.lines[i];
    if (!line.endpoint) {
      vertexMap.push(null);
      continue;
    }

    const x = line.endpoint[0] ?? 0;
    const y = line.endpoint[1] ?? 0;
    const z = line.endpoint[2] ?? 0;
    const moveType = line.move_type || '';

    if (moveType === 'Rapid') {
      const vStart = rapidPositions.length / 3;
      rapidPositions.push(prevX, prevY, prevZ, x, y, z);
      vertexMap.push({ target: 'rapid', vertexStart: vStart, vertexCount: 2 });
    } else if (moveType === 'Linear') {
      const vStart = feedPositions.length / 3;
      feedPositions.push(prevX, prevY, prevZ, x, y, z);
      vertexMap.push({ target: 'feed', vertexStart: vStart, vertexCount: 2 });
    } else if (moveType === 'ArcCW' || moveType === 'ArcCCW') {
      const vStart = feedPositions.length / 3;
      if (line.arc) {
        const arcPts = interpolateArc(
          prevX, prevY, prevZ, x, y, z,
          line.arc, moveType === 'ArcCW',
        );
        for (let j = 0; j < arcPts.length; j++) {
          feedPositions.push(arcPts[j]);
        }
      } else {
        feedPositions.push(prevX, prevY, prevZ, x, y, z);
      }
      const vCount = (feedPositions.length / 3) - vStart;
      vertexMap.push({ target: 'feed', vertexStart: vStart, vertexCount: vCount });
    } else {
      vertexMap.push(null);
    }

    prevX = x;
    prevY = y;
    prevZ = z;
  }

  // Build rapid lines with per-vertex colors
  if (rapidPositions.length > 0) {
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(rapidPositions, 3));
    const colors = new Float32Array(rapidPositions.length);
    for (let i = 0; i < colors.length; i += 3) {
      colors[i] = RAPID_COLOR[0];
      colors[i + 1] = RAPID_COLOR[1];
      colors[i + 2] = RAPID_COLOR[2];
    }
    geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    const mat = new THREE.LineBasicMaterial({
      vertexColors: true,
      transparent: true,
      opacity: 0.4,
      linewidth: 1,
    });
    rapidLines = new THREE.LineSegments(geom, mat);
    scene.add(rapidLines);
  }

  // Build feed lines with per-vertex colors
  if (feedPositions.length > 0) {
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(feedPositions, 3));
    const colors = new Float32Array(feedPositions.length);
    for (let i = 0; i < colors.length; i += 3) {
      colors[i] = FEED_COLOR[0];
      colors[i + 1] = FEED_COLOR[1];
      colors[i + 2] = FEED_COLOR[2];
    }
    geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    const mat = new THREE.LineBasicMaterial({
      vertexColors: true,
      linewidth: 1,
    });
    feedLines = new THREE.LineSegments(geom, mat);
    scene.add(feedLines);
  }

  // Auto-fit camera to bounding box
  if (file.bounding_box) {
    const [bbMin, bbMax] = file.bounding_box;
    const minX = bbMin[0] ?? 0, minY = bbMin[1] ?? 0, minZ = bbMin[2] ?? 0;
    const maxX = bbMax[0] ?? 0, maxY = bbMax[1] ?? 0, maxZ = bbMax[2] ?? 0;
    const center = new THREE.Vector3(
      (minX + maxX) / 2,
      (minY + maxY) / 2,
      (minZ + maxZ) / 2,
    );
    const size = Math.max(maxX - minX, maxY - minY, maxZ - minZ);
    const distance = size * 1.5;

    const cam = getCamera();
    const ctrl = getControls();
    if (cam && ctrl) {
      cam.position.set(center.x + distance, center.y + distance, center.z + distance * 0.7);
      ctrl.target.copy(center);
      ctrl.update();
    }
  }
}

/** Update toolpath colors to show progress. currentLine is 1-based count of completed lines. */
export function updateToolpathProgress(currentLine: number): void {
  // Detect new job (progress went backwards) and reset colors
  if (currentLine < lastProgressLine) {
    resetToolpathProgress();
  }
  if (currentLine <= lastProgressLine) return; // nothing new

  const rapidColorAttr = rapidLines?.geometry.getAttribute('color') as THREE.BufferAttribute | undefined;
  const feedColorAttr = feedLines?.geometry.getAttribute('color') as THREE.BufferAttribute | undefined;

  let rapidDirty = false;
  let feedDirty = false;

  for (let i = lastProgressLine < 0 ? 0 : lastProgressLine; i < currentLine && i < vertexMap.length; i++) {
    const m = vertexMap[i];
    if (!m) continue;

    const attr = m.target === 'rapid' ? rapidColorAttr : feedColorAttr;
    if (!attr) continue;

    const arr = attr.array as Float32Array;
    for (let v = 0; v < m.vertexCount; v++) {
      const idx = (m.vertexStart + v) * 3;
      arr[idx] = DONE_COLOR[0];
      arr[idx + 1] = DONE_COLOR[1];
      arr[idx + 2] = DONE_COLOR[2];
    }

    if (m.target === 'rapid') rapidDirty = true;
    else feedDirty = true;
  }

  if (rapidDirty && rapidColorAttr) rapidColorAttr.needsUpdate = true;
  if (feedDirty && feedColorAttr) feedColorAttr.needsUpdate = true;

  lastProgressLine = currentLine;
}

/** Reset progress coloring back to original colors */
export function resetToolpathProgress(): void {
  lastProgressLine = -1;

  const rapidColorAttr = rapidLines?.geometry.getAttribute('color') as THREE.BufferAttribute | undefined;
  const feedColorAttr = feedLines?.geometry.getAttribute('color') as THREE.BufferAttribute | undefined;

  if (rapidColorAttr) {
    const arr = rapidColorAttr.array as Float32Array;
    for (let i = 0; i < arr.length; i += 3) {
      arr[i] = RAPID_COLOR[0];
      arr[i + 1] = RAPID_COLOR[1];
      arr[i + 2] = RAPID_COLOR[2];
    }
    rapidColorAttr.needsUpdate = true;
  }

  if (feedColorAttr) {
    const arr = feedColorAttr.array as Float32Array;
    for (let i = 0; i < arr.length; i += 3) {
      arr[i] = FEED_COLOR[0];
      arr[i + 1] = FEED_COLOR[1];
      arr[i + 2] = FEED_COLOR[2];
    }
    feedColorAttr.needsUpdate = true;
  }
}

/** Return the bounding box of the toolpath geometry (rapid + feed lines) */
export function getToolpathBounds(): THREE.Box3 | null {
  const box = new THREE.Box3();
  let any = false;
  if (rapidLines) { box.expandByObject(rapidLines); any = true; }
  if (feedLines) { box.expandByObject(feedLines); any = true; }
  return any && !box.isEmpty() ? box : null;
}

/** Remove the highlight overlay line from the scene. */
function clearHighlight(): void {
  if (highlightLine) {
    const scene = getScene();
    if (scene) scene.remove(highlightLine);
    highlightLine.geometry.dispose();
    (highlightLine.material as LineMaterial).dispose();
    highlightLine = null;
  }
}

/** Highlight a toolpath line with a thick bright-yellow overlay. lineNum is 1-based. */
export function highlightToolpathLine(lineNum: number | null): void {
  clearHighlight();

  if (lineNum === null || lineNum < 1) return;

  const idx = lineNum - 1;
  if (idx >= vertexMap.length) return;
  const m = vertexMap[idx];
  if (!m) return;

  const obj = m.target === 'rapid' ? rapidLines : feedLines;
  const posAttr = obj?.geometry.getAttribute('position') as THREE.BufferAttribute | undefined;
  if (!posAttr) return;

  const renderer = getRenderer();
  const scene = getScene();
  if (!renderer || !scene) return;

  // Extract positions for this line segment from the source geometry
  // LineSegments uses vertex pairs: (v0,v1), (v2,v3), ...
  // Line2/LineGeometry wants a continuous polyline, so convert pairs to unique points
  const positions: number[] = [];
  const src = posAttr.array as Float32Array;
  for (let v = 0; v < m.vertexCount; v += 2) {
    const i0 = (m.vertexStart + v) * 3;
    if (v === 0) {
      positions.push(src[i0], src[i0 + 1], src[i0 + 2]);
    }
    const i1 = (m.vertexStart + v + 1) * 3;
    positions.push(src[i1], src[i1 + 1], src[i1 + 2]);
  }

  if (positions.length < 6) return; // need at least 2 points

  const geom = new LineGeometry();
  geom.setPositions(positions);

  const mat = new LineMaterial({
    color: 0xffff00,
    linewidth: 3, // pixels
    resolution: new THREE.Vector2(renderer.domElement.clientWidth, renderer.domElement.clientHeight),
    depthTest: true,
  });

  highlightLine = new Line2(geom, mat);
  highlightLine.computeLineDistances();
  scene.add(highlightLine);
}

/** Raycast from a click event against toolpath lines, set pickedLineNum signal. */
export function pickToolpathLine(event: MouseEvent): number | null {
  const renderer = getRenderer();
  const camera = getCamera();
  if (!renderer || !camera) return null;

  const rect = renderer.domElement.getBoundingClientRect();
  const ndc = new THREE.Vector2(
    ((event.clientX - rect.left) / rect.width) * 2 - 1,
    -((event.clientY - rect.top) / rect.height) * 2 + 1,
  );

  const raycaster = new THREE.Raycaster();
  raycaster.setFromCamera(ndc, camera);
  raycaster.params.Line!.threshold = 2;

  const targets: THREE.LineSegments[] = [];
  if (rapidLines) targets.push(rapidLines);
  if (feedLines) targets.push(feedLines);
  if (targets.length === 0) return null;

  const intersects = raycaster.intersectObjects(targets, false);
  if (intersects.length === 0) {
    setPickedLineNum(null);
    return null;
  }

  const hit = intersects[0];
  const hitObject = hit.object as THREE.LineSegments;
  const vertexIndex = hit.index;
  if (vertexIndex === undefined) {
    setPickedLineNum(null);
    return null;
  }

  const hitTarget: 'rapid' | 'feed' = hitObject === rapidLines ? 'rapid' : 'feed';

  for (let i = 0; i < vertexMap.length; i++) {
    const m = vertexMap[i];
    if (!m || m.target !== hitTarget) continue;
    if (vertexIndex >= m.vertexStart && vertexIndex < m.vertexStart + m.vertexCount) {
      const lineNum = i + 1;
      setPickedLineNum(lineNum);
      return lineNum;
    }
  }

  setPickedLineNum(null);
  return null;
}

export function clearToolpath(): void {
  const scene = getScene();
  if (!scene) return;

  clearHighlight();

  if (rapidLines) {
    scene.remove(rapidLines);
    rapidLines.geometry.dispose();
    (rapidLines.material as THREE.Material).dispose();
    rapidLines = null;
  }
  if (feedLines) {
    scene.remove(feedLines);
    feedLines.geometry.dispose();
    (feedLines.material as THREE.Material).dispose();
    feedLines = null;
  }

  vertexMap = [];
  lastProgressLine = -1;
}
