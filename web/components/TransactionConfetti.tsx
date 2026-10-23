'use client';

import React, { useEffect, useRef, useState } from 'react';

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

const CONFETTI_DURATION_MS = 3000;
const SOUND_TOGGLE_KEY = 'transaction-confetti-sound-enabled';

export const CONFETTI_CANVAS_STYLE: React.CSSProperties = {
  position: 'fixed',
  top: 0,
  left: 0,
  width: '100%',
  height: '100%',
  pointerEvents: 'none',
  zIndex: 9999,
};

export const TransactionConfetti: React.FC = () => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const particlesRef = useRef<Particle[]>([]);
  const animationFrameRef = useRef<number | null>(null);
  const cleanupTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const [active, setActive] = useState(false);
  const [soundEnabled, setSoundEnabled] = useState(true);
  // The global trigger is registered once on mount, so it reads the sound
  // preference through a ref instead of a stale closure.
  const soundEnabledRef = useRef(soundEnabled);
  soundEnabledRef.current = soundEnabled;

  useEffect(() => {
    try {
      const stored = window.localStorage.getItem(SOUND_TOGGLE_KEY);
      if (stored !== null) {
        setSoundEnabled(stored === 'true');
      }
    } catch {
      // localStorage may be unavailable in private mode
    }
  }, []);

  const playConfettiSound = () => {
    if (!soundEnabledRef.current) return;
    try {
      const AudioCtxtor =
        (window as any).AudioContext || (window as any).webkitAudioContext;
      if (!AudioCtxtor) return;
      if (!audioContextRef.current) {
        audioContextRef.current = new AudioCtxtor();
      }
      const audioCtx = audioContextRef.current;
      if (!audioCtx) return;
      if (audioCtx.state === 'suspended') {
        void audioCtx.resume();
      }
      const now = audioCtx.currentTime;
      const gain = audioCtx.createGain();
      gain.gain.setValueAtTime(0.0001, now);
      gain.gain.exponentialRampToValueAtTime(0.15, now + 0.01);
      gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.4);
      gain.connect(audioCtx.destination);

      const osc = audioCtx.createOscillator();
      osc.type = 'triangle';
      osc.frequency.setValueAtTime(880, now);
      osc.frequency.exponentialRampToValueAtTime(1760, now + 0.15);
      osc.connect(gain);
      osc.start(now);
      osc.stop(now + 0.4);
    } catch {
      // Audio is best-effort; ignore failures
    }
  };

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

  const triggerConfetti = () => {
    setActive(true);
    initParticles();
    playConfettiSound();

    if (cleanupTimerRef.current !== null) {
      clearTimeout(cleanupTimerRef.current);
    }
    cleanupTimerRef.current = setTimeout(() => {
      particlesRef.current = [];
      setActive(false);
      cleanupTimerRef.current = null;
    }, CONFETTI_DURATION_MS);
  };

  const toggleSound = () => {
    setSoundEnabled((prev) => {
      const next = !prev;
      try {
        window.localStorage.setItem(SOUND_TOGGLE_KEY, String(next));
      } catch {
        // localStorage may be unavailable
      }
      return next;
    });
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
      triggerConfetti();
    };

    return () => {
      window.removeEventListener('resize', handleResize);
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      if (cleanupTimerRef.current !== null) {
        clearTimeout(cleanupTimerRef.current);
        cleanupTimerRef.current = null;
      }
      particlesRef.current = [];
      delete (window as any).triggerConfetti;
    };
  }, []);

  return (
    <>
      {/*
        Decorative full-screen overlay. pointer-events: none lets clicks pass
        through to the page while the animation plays (#1283), and it is
        hidden from assistive tech and the hit-test tree when idle.
      */}
      <canvas
        ref={canvasRef}
        aria-hidden="true"
        data-testid="transaction-confetti-canvas"
        style={CONFETTI_CANVAS_STYLE}
        hidden={!active}
      />
      <button
        type="button"
        onClick={toggleSound}
        aria-pressed={soundEnabled}
        title={soundEnabled ? 'Disable confetti sound' : 'Enable confetti sound'}
        style={{
          position: 'fixed',
          bottom: 16,
          right: 16,
          zIndex: 10000,
          pointerEvents: 'auto',
          borderRadius: '9999px',
          border: '1px solid rgba(255,255,255,0.2)',
          background: 'rgba(15,23,42,0.85)',
          color: '#fff',
          padding: '8px 12px',
          cursor: 'pointer',
          fontSize: 12,
          lineHeight: 1,
        }}
      >
        {soundEnabled ? '🔉' : '🔇'}
      </button>
    </>
  );
}
