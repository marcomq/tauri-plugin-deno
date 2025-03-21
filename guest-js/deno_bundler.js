#!/usr/bin/env -S deno run --allow-env --allow-write --allow-run 

/** Tauri Plugin Deno
 * © Copyright 2025, by Marco Mengelkoch
 * Licensed under MIT License, see License file for more details
 * git clone https://github.com/marcomq/tauri-plugin-deno
**/

import * as esbuild from  "npm:esbuild@0.25.1";
import { denoPlugins } from "jsr:@luca/esbuild-deno-loader@0.11.1";

const result = await esbuild.build({
  plugins: [...denoPlugins()],
  entryPoints: ["./src-tauri/src-deno/main.js"],
  outfile: "./src-tauri/target/deno_dist.js",
  bundle: true,
  platform: "node",
  format: "esm",
  // minify: true,
  // sourcemap: true,
  treeShaking: true,
});
console.log("bundle finished: " + result.outputFiles);
await esbuild.stop();
globalThis.close();

// deno run --allow-read --allow-write --allow-env --allow-net --allow-run deno_bundler.js