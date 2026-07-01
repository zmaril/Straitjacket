import { fileURLToPath } from "node:url";
import { createMDX } from "fumadocs-mdx/next";

const withMDX = createMDX();

/** @type {import('next').NextConfig} */
const config = {
  output: "export",
  reactStrictMode: true,
  // Pin the workspace root so the build doesn't get confused by lockfiles in
  // parent directories (e.g. a stray ~/pnpm-lock.yaml).
  turbopack: {
    root: fileURLToPath(new URL(".", import.meta.url)),
  },
};

export default withMDX(config);
