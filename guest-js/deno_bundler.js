#!/usr/bin/env -S deno run --allow-env --allow-write --allow-run 

/** Tauri Plugin Deno
 * © Copyright 2025, by Marco Mengelkoch
 * Licensed under MIT License, see License file for more details
 * git clone https://github.com/marcomq/tauri-plugin-deno
**/

import * as esbuild from  "https://deno.land/x/esbuild@v0.25.1/mod.js";
import { denoPlugins } from "jsr:@luca/esbuild-deno-loader@0.11.1";

const entryPoint = (Deno.args.length == 1) ? Deno.args[0] :  "./src-tauri/src-deno/main.js";

let default_ctx = {
  plugins: [...denoPlugins()],
  entryPoints: [entryPoint],
  outfile: "./src-tauri/target/deno_dist.js",
  bundle: true,
  platform: "node",
  format: "esm",
  // minify: true,
  treeShaking: true
};

if (Deno.args.length > 1) {
  try {
    const json_args = JSON.parse(Deno.args[1]);
    default_ctx = {...default_ctx, ...json_args};
  }
  catch(e) {
    console.error("Error parsing json parameter, using default");
    console.error(e);
  }

}
const ctx = await esbuild.context(default_ctx);
await ctx.watch()
console.log(`Watching bundle of ${entryPoint}.`);

// deno run --allow-read --allow-write --allow-env --allow-net --allow-run deno_bundler.js