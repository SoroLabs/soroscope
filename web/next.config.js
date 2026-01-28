/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  transpilePackages: ["@creit.tech/stellar-wallets-kit"],
  turbopack: {},
};

module.exports = nextConfig;
