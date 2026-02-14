import * as THREE from 'three';
import { getScene } from './scene';

let toolMesh: THREE.Mesh | null = null;
let trailLine: THREE.Line | null = null;
let trailMaterial: THREE.LineBasicMaterial | null = null;
const trailPositions: number[] = [];
const MAX_TRAIL = 5000;

function getTrailMaterial(): THREE.LineBasicMaterial {
  if (!trailMaterial) {
    trailMaterial = new THREE.LineBasicMaterial({
      color: 0xff6644,
      transparent: true,
      opacity: 0.8,
    });
  }
  return trailMaterial;
}

export function updateToolPosition(x: number, y: number, z: number, isRunning: boolean): void {
  const scene = getScene();
  if (!scene) return;

  // Create tool mesh on first call
  if (!toolMesh) {
    const geom = new THREE.ConeGeometry(2, 8, 8);
    geom.rotateX(Math.PI); // point downward
    const mat = new THREE.MeshPhongMaterial({
      color: 0xff4444,
      emissive: 0x441111,
    });
    toolMesh = new THREE.Mesh(geom, mat);
    scene.add(toolMesh);
  }

  toolMesh.position.set(x, y, z + 4); // offset cone so tip is at Z

  // Update trail when running
  if (isRunning) {
    trailPositions.push(x, y, z);

    // Trim trail if too long
    if (trailPositions.length > MAX_TRAIL * 3) {
      trailPositions.splice(0, trailPositions.length - MAX_TRAIL * 3);
    }

    if (trailPositions.length >= 6) {
      if (trailLine) {
        scene.remove(trailLine);
        trailLine.geometry.dispose();
      }

      const geom = new THREE.BufferGeometry();
      geom.setAttribute('position', new THREE.Float32BufferAttribute(trailPositions, 3));
      trailLine = new THREE.Line(geom, getTrailMaterial());
      scene.add(trailLine);
    }
  }
}

export function clearTrail(): void {
  const scene = getScene();
  if (!scene) return;

  trailPositions.length = 0;
  if (trailLine) {
    scene.remove(trailLine);
    trailLine.geometry.dispose();
    trailLine = null;
  }
}

export function disposeTool(): void {
  if (toolMesh) {
    toolMesh.geometry.dispose();
    (toolMesh.material as THREE.Material).dispose();
    toolMesh = null;
  }
  clearTrail();
  if (trailMaterial) {
    trailMaterial.dispose();
    trailMaterial = null;
  }
}
