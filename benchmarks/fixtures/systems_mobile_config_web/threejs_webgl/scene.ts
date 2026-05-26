import * as THREE from 'three';

const scene = new THREE.Scene();
const loader = new GLTFLoader();
loader.load('/models/ship.glb');
requestAnimationFrame(tick);
