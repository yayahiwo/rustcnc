import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { createGrid } from './grid';
import { disposeTool } from './tool';
import { clearToolpath } from './toolpath';

let renderer: THREE.WebGLRenderer | null = null;
let scene: THREE.Scene | null = null;
let camera: THREE.PerspectiveCamera | null = null;
let controls: OrbitControls | null = null;
let animationId: number | null = null;

export function getScene(): THREE.Scene | null {
  return scene;
}

export function getCamera(): THREE.PerspectiveCamera | null {
  return camera;
}

export function createScene(container: HTMLDivElement): () => void {
  const width = container.clientWidth;
  const height = container.clientHeight;

  // Renderer
  renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
  renderer.setSize(width, height);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setClearColor(0x0a0a0f);
  container.appendChild(renderer.domElement);

  // Scene
  scene = new THREE.Scene();

  // Camera
  camera = new THREE.PerspectiveCamera(50, width / height, 0.1, 10000);
  camera.position.set(150, 150, 200);
  camera.up.set(0, 0, 1); // Z-up for CNC

  // Controls
  controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.dampingFactor = 0.1;
  controls.target.set(0, 0, 0);

  // Lighting
  const ambient = new THREE.AmbientLight(0xffffff, 0.6);
  scene.add(ambient);

  const directional = new THREE.DirectionalLight(0xffffff, 0.8);
  directional.position.set(100, 100, 200);
  scene.add(directional);

  // Grid
  const grid = createGrid();
  scene.add(grid);

  // Origin axes
  const axesHelper = new THREE.AxesHelper(30);
  scene.add(axesHelper);

  // Animation loop
  const animate = () => {
    animationId = requestAnimationFrame(animate);
    if (controls) controls.update();
    if (renderer && scene && camera) {
      renderer.render(scene, camera);
    }
  };
  animate();

  // Resize observer
  const resizeObserver = new ResizeObserver(() => {
    const w = container.clientWidth;
    const h = container.clientHeight;
    if (w === 0 || h === 0) return;
    if (camera) {
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
    }
    if (renderer) {
      renderer.setSize(w, h);
    }
  });
  resizeObserver.observe(container);

  return () => {
    resizeObserver.disconnect();
  };
}

export function disposeScene(): void {
  if (animationId !== null) {
    cancelAnimationFrame(animationId);
    animationId = null;
  }
  if (controls) {
    controls.dispose();
    controls = null;
  }
  // Clean up tool and toolpath module globals
  disposeTool();
  clearToolpath();
  // Dispose all geometries and materials
  if (scene) {
    scene.traverse((obj: any) => {
      if (obj.geometry) obj.geometry.dispose();
      if (obj.material) {
        if (Array.isArray(obj.material)) {
          obj.material.forEach((m: any) => m.dispose());
        } else {
          obj.material.dispose();
        }
      }
    });
    scene = null;
  }
  if (renderer) {
    renderer.dispose();
    renderer.domElement.remove();
    renderer = null;
  }
  camera = null;
}

export function resetCamera(): void {
  if (camera && controls) {
    camera.position.set(150, 150, 200);
    controls.target.set(0, 0, 0);
    controls.update();
  }
}

export function zoomCamera(factor: number): void {
  if (!camera || !controls) return;
  const dir = new THREE.Vector3()
    .subVectors(camera.position, controls.target)
    .multiplyScalar(1 - factor);
  camera.position.sub(dir);
  controls.update();
}

export function fitToScene(): void {
  if (!scene || !camera || !controls) return;
  const box = new THREE.Box3();
  scene.traverse((obj) => {
    if (obj instanceof THREE.LineSegments) {
      box.expandByObject(obj);
    }
  });
  if (box.isEmpty()) {
    resetCamera();
    return;
  }
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3());
  const maxDim = Math.max(size.x, size.y, size.z);
  const distance = maxDim * 1.5;
  camera.position.set(
    center.x + distance,
    center.y + distance,
    center.z + distance * 0.7,
  );
  controls.target.copy(center);
  controls.update();
}

export type ViewPreset = 'top' | 'front' | 'right' | '3d';

export function setCameraView(preset: ViewPreset): void {
  if (!camera || !controls) return;
  const target = controls.target.clone();
  const dist = camera.position.distanceTo(target);
  switch (preset) {
    case 'top':
      camera.position.set(target.x, target.y, target.z + dist);
      camera.up.set(0, 1, 0);
      break;
    case 'front':
      camera.position.set(target.x, target.y - dist, target.z);
      camera.up.set(0, 0, 1);
      break;
    case 'right':
      camera.position.set(target.x + dist, target.y, target.z);
      camera.up.set(0, 0, 1);
      break;
    case '3d':
      camera.position.set(
        target.x + dist * 0.577,
        target.y + dist * 0.577,
        target.z + dist * 0.577,
      );
      camera.up.set(0, 0, 1);
      break;
  }
  controls.update();
}

export function getControls(): OrbitControls | null {
  return controls;
}
