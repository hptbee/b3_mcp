import * as THREE from 'three';

const canvas = document.getElementById('stage');
const renderer = new THREE.WebGLRenderer({ canvas });
const light = new THREE.DirectionalLight();
const material = new THREE.ShaderMaterial();
new GLTFLoader().load('/models/ship.glb');
const shader = '/shaders/water.frag';
requestAnimationFrame(tick);
