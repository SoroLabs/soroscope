declare module 'mermaid' {
  interface MermaidConfig {
    startOnLoad?: boolean;
    theme?: string;
    securityLevel?: string;
    [key: string]: any;
  }
  const mermaid: {
    initialize(config: MermaidConfig): void;
    render(id: string, text: string): Promise<{ svg: string }>;
    run(config?: { querySelector?: string }): void;
  };
  export default mermaid;
}