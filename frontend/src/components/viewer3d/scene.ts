import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { createGrid } from './grid';
import { disposeTool } from './tool';
import { clearToolpath, getToolpathBounds } from './toolpath';

function createTextSprite(text: string, color: string): THREE.Sprite {
  const size = 64;
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d')!;
  ctx.font = 'bold 48px sans-serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillStyle = color;
  ctx.fillText(text, size / 2, size / 2);
  const texture = new THREE.CanvasTexture(canvas);
  texture.minFilter = THREE.LinearFilter;
  const mat = new THREE.SpriteMaterial({ map: texture, depthTest: false });
  const sprite = new THREE.Sprite(mat);
  sprite.scale.set(8, 8, 1);
  return sprite;
}

let renderer: THREE.WebGLRenderer | null = null;
let scene: THREE.Scene | null = null;
let perspCamera: THREE.PerspectiveCamera | null = null;
let orthoCamera: THREE.OrthographicCamera | null = null;
let activeCamera: THREE.PerspectiveCamera | THREE.OrthographicCamera | null = null;
let controls: OrbitControls | null = null;
let animationId: number | null = null;

export function getScene(): THREE.Scene | null {
  return scene;
}

export function getCamera(): THREE.PerspectiveCamera | THREE.OrthographicCamera | null {
  return activeCamera;
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

  // Perspective Camera
  perspCamera = new THREE.PerspectiveCamera(50, width / height, 0.1, 10000);
  perspCamera.position.set(150, 150, 200);
  perspCamera.up.set(0, 0, 1); // Z-up for CNC

  // Orthographic Camera (initial frustum; recalculated on view switch)
  const aspect = width / height;
  const frustumSize = 300;
  orthoCamera = new THREE.OrthographicCamera(
    -frustumSize * aspect / 2, frustumSize * aspect / 2,
    frustumSize / 2, -frustumSize / 2,
    0.1, 10000,
  );
  orthoCamera.up.set(0, 0, 1);

  // Start with perspective
  activeCamera = perspCamera;

  // Controls
  controls = new OrbitControls(activeCamera, renderer.domElement);
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

  // Axis labels
  const axisLabels: [string, THREE.Vector3, string][] = [
    ['X', new THREE.Vector3(34, 0, 0), '#ef4444'],
    ['Y', new THREE.Vector3(0, 34, 0), '#34d399'],
    ['Z', new THREE.Vector3(0, 0, 34), '#4a9eff'],
  ];
  for (const [letter, pos, color] of axisLabels) {
    const sprite = createTextSprite(letter, color);
    sprite.position.copy(pos);
    scene.add(sprite);
  }

  // Animation loop
  const animate = () => {
    animationId = requestAnimationFrame(animate);
    if (controls) controls.update();
    if (renderer && scene && activeCamera) {
      renderer.render(scene, activeCamera);
    }
  };
  animate();

  // Resize observer
  const resizeObserver = new ResizeObserver(() => {
    const w = container.clientWidth;
    const h = container.clientHeight;
    if (w === 0 || h === 0) return;
    const a = w / h;
    if (perspCamera) {
      perspCamera.aspect = a;
      perspCamera.updateProjectionMatrix();
    }
    if (orthoCamera) {
      const halfH = (orthoCamera.top - orthoCamera.bottom) / 2;
      orthoCamera.left = -halfH * a;
      orthoCamera.right = halfH * a;
      orthoCamera.updateProjectionMatrix();
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
  perspCamera = null;
  orthoCamera = null;
  activeCamera = null;
}

export function resetCamera(): void {
  if (perspCamera && controls) {
    activeCamera = perspCamera;
    controls.object = perspCamera;
    controls.enableRotate = true;
    perspCamera.position.set(150, 150, 200);
    perspCamera.up.set(0, 0, 1);
    controls.target.set(0, 0, 0);
    controls.update();
  }
}

export function zoomCamera(factor: number): void {
  if (!activeCamera || !controls) return;
  if (activeCamera instanceof THREE.OrthographicCamera) {
    // For ortho, scale the frustum
    const zoom = 1 + factor;
    activeCamera.left /= zoom;
    activeCamera.right /= zoom;
    activeCamera.top /= zoom;
    activeCamera.bottom /= zoom;
    activeCamera.updateProjectionMatrix();
    controls.update();
  } else {
    // For perspective, call OrbitControls' internal dolly methods —
    // same code path as scroll-wheel zoom
    const ctrl = controls as any;
    const dollyScale = Math.pow(0.95, Math.abs(factor) * 10);
    if (factor > 0) {
      ctrl._dollyIn(dollyScale);
    } else {
      ctrl._dollyOut(dollyScale);
    }
    controls.update();
  }
}

export function fitToScene(): void {
  if (!scene || !activeCamera || !controls) return;
  const box = getToolpathBounds();
  if (!box) {
    resetCamera();
    return;
  }
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3());
  const maxDim = Math.max(size.x, size.y, size.z);
  const distance = maxDim * 1.2;

  if (activeCamera instanceof THREE.OrthographicCamera && renderer) {
    const aspect = renderer.domElement.clientWidth / renderer.domElement.clientHeight;
    const halfH = maxDim * 0.55;
    activeCamera.left = -halfH * aspect;
    activeCamera.right = halfH * aspect;
    activeCamera.top = halfH;
    activeCamera.bottom = -halfH;
    activeCamera.updateProjectionMatrix();
    // Position along current viewing axis
    const dir = new THREE.Vector3().subVectors(activeCamera.position, controls.target).normalize();
    activeCamera.position.copy(center).addScaledVector(dir, distance);
  } else {
    activeCamera.position.set(
      center.x + distance,
      center.y + distance,
      center.z + distance * 0.7,
    );
  }
  controls.target.copy(center);
  controls.update();
}

export type ViewPreset = 'top' | 'front' | 'right' | '3d';

export function setCameraView(preset: ViewPreset): void {
  if (!controls || !perspCamera || !orthoCamera || !renderer) return;
  const target = controls.target.clone();
  const dist = (activeCamera || perspCamera).position.distanceTo(target);

  if (preset === '3d') {
    // Switch to perspective camera
    activeCamera = perspCamera;
    controls.object = perspCamera;
    controls.enableRotate = true;
    perspCamera.position.set(
      target.x + dist * 0.577,
      target.y + dist * 0.577,
      target.z + dist * 0.577,
    );
    perspCamera.up.set(0, 0, 1);
  } else {
    // Switch to orthographic camera
    const aspect = renderer.domElement.clientWidth / renderer.domElement.clientHeight;
    const fov = perspCamera.fov * Math.PI / 180;
    const frustumHeight = 2 * dist * Math.tan(fov / 2);
    orthoCamera.left = -frustumHeight * aspect / 2;
    orthoCamera.right = frustumHeight * aspect / 2;
    orthoCamera.top = frustumHeight / 2;
    orthoCamera.bottom = -frustumHeight / 2;
    orthoCamera.updateProjectionMatrix();

    activeCamera = orthoCamera;
    controls.object = orthoCamera;
    controls.enableRotate = false;

    switch (preset) {
      case 'top':
        orthoCamera.position.set(target.x, target.y, target.z + dist);
        orthoCamera.up.set(0, 1, 0);
        break;
      case 'front':
        orthoCamera.position.set(target.x, target.y - dist, target.z);
        orthoCamera.up.set(0, 0, 1);
        break;
      case 'right':
        orthoCamera.position.set(target.x + dist, target.y, target.z);
        orthoCamera.up.set(0, 0, 1);
        break;
    }
  }
  controls.update();
}

export function getControls(): OrbitControls | null {
  return controls;
}

export function getRenderer(): THREE.WebGLRenderer | null {
  return renderer;
}
