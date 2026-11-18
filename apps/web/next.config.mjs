/** @type {import('next').NextConfig} */
const nextConfig = {
  // The Tauri shell serves a static bundle from `out/`; there is no Node server in production.
  output: 'export',
  images: { unoptimized: true },
  trailingSlash: true,
};

export default nextConfig;
