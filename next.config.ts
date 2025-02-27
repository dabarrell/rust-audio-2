import type { NextConfig } from "next";
import WasmPackPlugin from "@wasm-tool/wasm-pack-plugin";
import path from "path";

const nextConfig: NextConfig = {
  webpack: (config) => {
    config.experiments = {
      ...config.experiments,
      asyncWebAssembly: true,
    };

    config.plugins.push(
      new WasmPackPlugin({
        crateDirectory: path.resolve(__dirname, "crate"),
        outDir: path.resolve(__dirname, "crate/pkg"),
      })
    );

    return config;
  },
};

export default nextConfig;
