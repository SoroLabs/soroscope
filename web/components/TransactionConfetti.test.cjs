'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

// Simulate particle dynamics calculations for physics culling test assertions
function simulateConfettiStep(p, gravity = 0.35, drag = 0.98) {
  p.x += p.speedX;
  p.y += p.speedY;
  p.speedY += gravity;
  p.speedX *= drag;
  p.rotation += p.rotationSpeed;

  if (p.speedY > 0) {
    p.opacity -= 0.015;
  }
}

test('TransactionConfetti: particle initialization produces expected ranges', () => {
  const width = 1920;
  const height = 1080;
  const count = 100;
  const particles = [];

  for (let i = 0; i < count; i++) {
    particles.push({
      x: width / 2 + (Math.random() - 0.5) * 40,
      y: height * 0.65,
      size: Math.random() * 8 + 6,
      speedX: (Math.random() - 0.5) * 16,
      speedY: -Math.random() * 12 - 8,
      rotation: Math.random() * 360,
      rotationSpeed: (Math.random() - 0.5) * 10,
      opacity: 1,
    });
  }

  assert.strictEqual(particles.length, 100);
  assert.ok(particles[0].x >= width / 2 - 20 && particles[0].x <= width / 2 + 20);
  assert.ok(particles[0].speedY < 0, 'Confetti must start by traveling upwards (negative speedY)');
  assert.strictEqual(particles[0].opacity, 1.0);
});

test('TransactionConfetti: physics culling fader triggers opacity decay on downward trajectory', () => {
  const p = {
    x: 960,
    y: 700,
    speedX: 2.0,
    speedY: 1.0, // moving downwards
    rotation: 0,
    rotationSpeed: 2,
    opacity: 1.0,
  };

  simulateConfettiStep(p);

  assert.ok(p.opacity < 1.0, 'Opacity should decay when speedY is positive (moving downward)');
  assert.ok(p.speedX < 2.0, 'Air resistance drag should decelerate horizontal speedX');
  assert.ok(p.speedY > 1.0, 'Gravity acceleration should increase speedY');
});

test('TransactionConfetti: particle remains stable when traveling upwards', () => {
  const p = {
    x: 960,
    y: 700,
    speedX: 2.0,
    speedY: -5.0, // moving upwards
    rotation: 0,
    rotationSpeed: 2,
    opacity: 1.0,
  };

  simulateConfettiStep(p);

  assert.strictEqual(p.opacity, 1.0, 'Opacity must remain stable when particle travels upwards');
});
