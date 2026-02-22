import * as THREE from 'three';
import { getScene } from './scene';

let toolMesh: THREE.Mesh | null = null;

export function updateToolPosition(x: number, y: number, z: number, _isRunning: boolean): void {
  const scene = getScene();
  if (!scene) return;

  // Create tool mesh on first call
  if (!toolMesh) {
    const geom = new THREE.ConeGeometry(2, 8, 8);
    geom.rotateX(-Math.PI / 2); // tip toward -Z
    geom.translate(0, 0, 4);    // shift so tip is at mesh origin
    const mat = new THREE.MeshPhongMaterial({
      color: 0xff4444,
      emissive: 0x441111,
    });
    toolMesh = new THREE.Mesh(geom, mat);
    scene.add(toolMesh);
  }

  toolMesh.position.set(x, y, z);
}

export function disposeTool(): void {
  if (toolMesh) {
    toolMesh.geometry.dispose();
    (toolMesh.material as THREE.Material).dispose();
    toolMesh = null;
  }
}
