/** @type {import('next').NextConfig} */
const isProd = process.env.NODE_ENV === "production";

/**
 * Content-Security-Policy for the Pages-router dashboard.
 *
 * The app (Next.js Pages router) injects an inline bootstrap script and
 * styled-jsx `<style>` blocks, and power users can point the UI at custom
 * self-hosted RPC / indexer endpoints, so `'unsafe-inline'` script+style and
 * `https:` connect-src are required to avoid breaking those flows. The CSP
 * still hardens the app: it blocks third-party script origins, `object`/`embed`
 * payloads, clickjacking (`frame-ancestors`) and form POST targets, and is
 * combined with `X-Frame-Options` / `X-Content-Type-Options` below.
 */
function buildContentSecurityPolicy() {
  const directives = [
    "default-src 'self'",
    "base-uri 'self'",
    "object-src 'none'",
    "frame-src 'none'",
    "frame-ancestors 'none'",
    "form-action 'self'",
    "script-src 'self' 'unsafe-inline' 'unsafe-eval'",
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data: blob: https://stellar.creit.tech https://grainy-gradients.vercel.app",
    "font-src 'self' data:",
    "connect-src 'self' https: wss: ws: http://localhost:*",
    "worker-src 'self' blob:",
    "manifest-src 'self'",
  ];
  if (isProd) {
    directives.push("upgrade-insecure-requests");
  }
  return directives.join("; ");
}

const securityHeaders = [
  {
    key: "Content-Security-Policy",
    value: buildContentSecurityPolicy(),
  },
  {
    key: "X-Frame-Options",
    value: "DENY",
  },
  {
    key: "X-Content-Type-Options",
    value: "nosniff",
  },
  {
    key: "Referrer-Policy",
    value: "strict-origin-when-cross-origin",
  },
  {
    key: "Permissions-Policy",
    value: "camera=(), microphone=(), geolocation=()",
  },
  ...(isProd
    ? [
        {
          key: "Strict-Transport-Security",
          value: "max-age=31536000; includeSubDomains",
        },
      ]
    : []),
];

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  transpilePackages: ["@creit.tech/stellar-wallets-kit"],
  webpack: (config, { isServer }) => {
    if (!isServer) {
      config.resolve.fallback = {
        ...config.resolve.fallback,
        fs: false,
        net: false,
        tls: false,
      };
    }
    return config;
  },
  async headers() {
    return [
      {
        source: "/:path*",
        headers: securityHeaders,
      },
    ];
  },
};

module.exports = nextConfig;