import * as THREE from 'three';

export function createGrid(): THREE.Group {
  const group = new THREE.Group();

  // Main grid on XY plane (Z=0 for CNC)
  const gridSize = 500;
  const gridDivisions = 50;

  const grid = new THREE.GridHelper(gridSize, gridDivisions, 0x333333, 0x1a1a2a);
  // Rotate so grid lies on XY plane (CNC convention: Z is up)
  grid.rotation.x = Math.PI / 2;
  group.add(grid);

  // Smaller 10mm grid overlay in the center
  const fineGrid = new THREE.GridHelper(100, 100, 0x222233, 0x15152a);
  fineGrid.rotation.x = Math.PI / 2;
  group.add(fineGrid);

  return group;
}
