import type { NextConfig } from "next";
import WasmPackPlugin from "@wasm-tool/wasm-pack-plugin";
import path from "path";

const nextConfig: NextConfig = {
  reactStrictMode: false,
  webpack: (config, { isServer }) => {
    config.experiments = {
      ...config.experiments,
      asyncWebAssembly: true, // TODO: what does this mean?
      // syncWebAssembly: true,
    };

    // TODO: Reenable this once we know the build is working, might need to bring over some extra args etc
    // RUSTFLAGS?
    // if (!isServer) {
    // config.plugins.push(
    //   new WasmPackPlugin({
    //     crateDirectory: path.resolve(__dirname, "crate"),
    //     outDir: path.resolve(__dirname, "crate/pkg"),
    //     extraArgs: "--target web",

    //   })
    // );
    // }

    return config;
  },
  async headers() {
    return [
      {
        // These two headers allow for SharedArrayBuffer, which is required for ffmpeg.wasm.
        // ffmpeg.wasm is only used on the upload page
        // https://developer.chrome.com/blog/enabling-shared-array-buffer/#cross-origin-isolation
        source: '/(.*)',
        headers: [
          {
            key: 'Cross-Origin-Opener-Policy',
            value: 'same-origin',
          },
          {
            key: 'Cross-Origin-Embedder-Policy',
            value: 'require-corp',
          },
        ],
      },
    ]
  }
};

export default nextConfig;
