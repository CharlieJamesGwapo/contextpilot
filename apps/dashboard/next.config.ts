import type { NextConfig } from 'next';
const config: NextConfig = { output: 'export', ...(process.env.NEXT_PUBLIC_CONTEXTPILOT_MODE==='hosted'?{distDir:'hosted-out'}:{}), trailingSlash: true, images: { unoptimized: true } };
export default config;
