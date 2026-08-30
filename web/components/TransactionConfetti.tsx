'use client';

import React, { useEffect, useRef } from 'react';

interface Particle {
  x: number;
  y: number;
  size: number;
  color: string;
  speedX: number;
  speedY: number;
  rotation: number;
  rotationSpeed: number;
  opacity: number;
}

const COLORS = [
  '#00d9ff', // Cyan
  '#34d399', // Emerald
  '#a78bfa', // Purple
  '#fb7185', // Rose
  '#fbbf24', // Amber
  '#38bdf8', // Light Blue
];

export const TransactionConfetti: React.FC = () => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const particlesRef = useRef<Particle[]>([]);
  const animationFrameRef = useRef<number | null>(null);

  const initParticles = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const count = 100;
    const particles: Particle[] = [];

    for (let i = 0; i < count; i++) {
      particles.push({
        x: canvas.width / 2 + (Math.random() - 0.5) * 40,
        y: canvas.height * 0.65,
        size: Math.random() * 8 + 6,
        color: COLORS[Math.floor(Math.random() * COLORS.length)],
        speedX: (Math.random() - 0.5) * 16,
        speedY: -Math.random() * 12 - 8,
        rotation: Math.random() * 360,
        rotationSpeed: (Math.random() - 0.5) * 10,
        opacity: 1,
      });
    }

    particlesRef.current = [...particlesRef.current, ...particles];
  };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const handleResize = () => {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    };

    handleResize();
    window.addEventListener('resize', handleResize);

    const render = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      const particles = particlesRef.current;
      const nextParticles: Particle[] = [];

      for (let i = 0; i < particles.length; i++) {
        const p = particles[i];

        // Apply physics
        p.x += p.speedX;
        p.y += p.speedY;
        p.speedY += 0.35; // Gravity
        p.speedX *= 0.98; // Air resistance
        p.rotation += p.rotationSpeed;

        // Start fading when falling
        if (p.speedY > 0) {
          p.opacity -= 0.015;
        }

        // Keep particle if it's within viewport and has visibility
        if (p.y < canvas.height && p.opacity > 0) {
          ctx.save();
          ctx.translate(p.x, p.y);
          ctx.rotate((p.rotation * Math.PI) / 180);
          ctx.fillStyle = p.color;
          ctx.globalAlpha = p.opacity;

          // Alternate rendering rectangles and circles
          if (i % 2 === 0) {
            ctx.fillRect(-p.size / 2, -p.size / 2, p.size, p.size);
          } else {
            ctx.beginPath();
            ctx.arc(0, 0, p.size / 2, 0, Math.PI * 2);
            ctx.fill();
          }

          ctx.restore();
          nextParticles.push(p);
        }
      }

      particlesRef.current = nextParticles;
      animationFrameRef.current = requestAnimationFrame(render);
    };

    animationFrameRef.current = requestAnimationFrame(render);

    // Register global trigger
    (window as any).triggerConfetti = () => {
      initParticles();
    };

    return () => {
      window.removeEventListener('resize', handleResize);
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      delete (window as any).triggerConfetti;
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        pointerEvents: 'none',
        zIndex: 9999,
      }}
    />
  );
};
