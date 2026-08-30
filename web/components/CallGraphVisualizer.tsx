'use client';

import React, { useEffect, useRef } from 'react';
import mermaid from 'mermaid';
import { sanitizeMermaidDefinition } from '../lib/security';

interface CallGraphVisualizerProps {
  mermaidDefinition: string;
}

export function CallGraphVisualizer({ mermaidDefinition }: CallGraphVisualizerProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    mermaid.initialize({
      startOnLoad: false,
      theme: 'dark',
      // `strict` sanitizes any HTML in labels/definitions instead of trusting
      // it, closing the XSS sink that `loose` + `htmlLabels` previously opened.
      securityLevel: 'strict',
      flowchart: {
        useMaxWidth: true,
        htmlLabels: false,
        curve: 'basis',
      },
    });
  }, []);

  useEffect(() => {
    let cancelled = false;

    const renderMermaid = async () => {
      const container = containerRef.current;
      if (!container || !mermaidDefinition) return;

      try {
        const sanitized = sanitizeMermaidDefinition(mermaidDefinition);
        if (!sanitized) return;

        const { svg } = await mermaid.render('mermaid-graph-' + Date.now(), sanitized);
        if (cancelled) return;
        container.innerHTML = '';
        container.innerHTML = svg;
      } catch (error) {
        console.error('Mermaid rendering failed:', error);
        if (cancelled || !containerRef.current) return;

        // Build the error node with the DOM APIs so the error text can never
        // be interpreted as HTML.
        containerRef.current.innerHTML = '';
        const message = document.createElement('p');
        message.style.color = '#fb8500';
        const label = document.createElement('span');
        label.textContent = 'Failed to render call graph: ';
        const detail = document.createElement('span');
        detail.textContent = error instanceof Error ? error.message : String(error);
        message.appendChild(label);
        message.appendChild(detail);
        containerRef.current.appendChild(message);
      }
    };

    renderMermaid();

    return () => {
      cancelled = true;
    };
  }, [mermaidDefinition]);

  return (
    <div style={{ marginTop: '20px' }}>
      <h4 style={{ color: '#00d9ff', fontSize: '14px', marginBottom: '12px', fontWeight: '600' }}>
        Cross-Contract Dependency Graph
      </h4>
      <div
        ref={containerRef}
        style={{
          backgroundColor: '#010409',
          padding: '16px',
          borderRadius: '8px',
          border: '1px solid #30363d',
          overflow: 'auto',
          minHeight: '100px',
        }}
      />
    </div>
  );
}