import { fileURLToPath } from "node:url";
import type { NextConfig } from "next";

const repositoryRoot = fileURLToPath(new URL("../..", import.meta.url));

const nextConfig: NextConfig = {
	output: "standalone",
	turbopack: {
		root: repositoryRoot,
	},
	experimental: {
		webpackMemoryOptimizations: true,
	},
};

export default nextConfig;
