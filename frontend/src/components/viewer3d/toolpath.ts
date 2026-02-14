import * as THREE from 'three';
import { getScene, getCamera } from './scene';
import type { GCodeFileInfo } from '../../lib/types';

let rapidLines: THREE.LineSegments | null = null;
let feedLines: THREE.LineSegments | null = null;

const RAPID_COLOR = new THREE.Color(0.3, 0.3, 0.5); // dim blue-gray
const FEED_COLOR = new THREE.Color(0.2, 0.8, 0.4);  // green

export function updateToolpath(file: GCodeFileInfo): void {
  const scene = getScene();
  if (!scene) return;

  // Remove old paths
  clearToolpath();

  const rapidPositions: number[] = [];
  const feedPositions: number[] = [];

  let prevX = 0, prevY = 0, prevZ = 0;

  for (const line of file.lines) {
    if (!line.endpoint) continue;

    const [x, y, z] = line.endpoint;
    const moveType = line.move_type || '';

    if (moveType === 'Rapid') {
      rapidPositions.push(prevX, prevY, prevZ, x, y, z);
    } else if (moveType === 'Linear' || moveType === 'CwArc' || moveType === 'CcwArc') {
      feedPositions.push(prevX, prevY, prevZ, x, y, z);
    }

    prevX = x;
    prevY = y;
    prevZ = z;
  }

  // Create rapid move lines
  if (rapidPositions.length > 0) {
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(rapidPositions, 3));
    const mat = new THREE.LineBasicMaterial({
      color: RAPID_COLOR,
      transparent: true,
      opacity: 0.4,
      linewidth: 1,
    });
    rapidLines = new THREE.LineSegments(geom, mat);
    scene.add(rapidLines);
  }

  // Create feed move lines
  if (feedPositions.length > 0) {
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(feedPositions, 3));
    const mat = new THREE.LineBasicMaterial({
      color: FEED_COLOR,
      linewidth: 1,
    });
    feedLines = new THREE.LineSegments(geom, mat);
    scene.add(feedLines);
  }

  // Auto-fit camera to bounding box
  if (file.bounding_box) {
    const [[minX, minY, minZ], [maxX, maxY, maxZ]] = file.bounding_box;
    const center = new THREE.Vector3(
      (minX + maxX) / 2,
      (minY + maxY) / 2,
      (minZ + maxZ) / 2,
    );
    const size = Math.max(maxX - minX, maxY - minY, maxZ - minZ);
    const distance = size * 1.5;

    const cam = getCamera();
    if (cam) {
      cam.position.set(center.x + distance, center.y + distance, center.z + distance * 0.7);
      cam.lookAt(center);
    }
  }
}

export function clearToolpath(): void {
  const scene = getScene();
  if (!scene) return;

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
}
