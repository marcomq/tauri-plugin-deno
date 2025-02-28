/** Tauri Plugin Deno
 * © Copyright 2025, by Marco Mengelkoch
 * Licensed under MIT License, see License file for more details
 * git clone https://github.com/marcomq/tauri-plugin-deno
**/

import { invoke } from '@tauri-apps/api/core'

export let call: { [index: string]: Function } = {}; // array of functions

export async function runCode(code: string): Promise<string> {
  return await invoke<{ value: string }>('plugin:deno|run_code', {
    payload: {
      value: code,
    },
  }).then((r: any) => {
    return r.value;
  });
}

/** 
 * Registers function on server and makes it available via `call.{jsFunctionName}`
 *  @param {string} denoFunctionCall - The deno js function call
 *  @param {number} [numberOfArgs] - Number of arguments, used for validation in deno, use -1 to ignore this value
 *  @param {string} [jsFunctionName] - Name that is used in javscript: "call.jsFunctionName". Must not contain dots.
 */
export async function registerFunction(
  denoFunctionCall: string,
  numberOfArgs?: number,
  jsFunctionName?: string): Promise<string> {
  if (numberOfArgs !== undefined && numberOfArgs < 0) {
    numberOfArgs = undefined;
  }
  return await invoke<{ value: string }>('plugin:deno|register_function', {
    payload: {
      denoFunctionCall,
      numberOfArgs
    },
  }).then((r: any) => {
    registerJs(denoFunctionCall, jsFunctionName);
    return r.value;
  });
}

/** 
 * No server invocation - assumes that function has already been registered server-side
 * Makes function available as `call.{jsFunctionName}`
 *  @param {string} denoFunctionCall - The deno js function call
 *  @param {string} [jsFunctionName] - Name that is used in javascript: "call.jsFunctionName". Must not contain dots.
 */
export async function registerJs(denoFunctionCall: string, jsFunctionName?: string) {
  if (jsFunctionName === undefined) {
    jsFunctionName = denoFunctionCall.replaceAll(".", "_");
  }
  call[jsFunctionName] = function (...args: any[]) { return callFunction(denoFunctionCall, args) };
}

/**
 * Calling previously registered function 
 */
export async function callFunction(functionName: string, args: any[]): Promise<string> {
  return invoke<{ value: string }>('plugin:deno|call_function', {
    payload: {
      functionName,
      args,
    },
  }).then((r: any) => {
    return r.value;
  });
}

/**
 * Read variable name from deno
 */
export async function readVariable(value: string): Promise<string> {
  return invoke<{ value: string }>('plugin:deno|read_variable', {
    payload: {
      value,
    },
  }).then((r: any) => {
    return r.value;
  });
}
