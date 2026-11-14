/** Tauri Plugin Deno
 * © Copyright 2025, by Marco Mengelkoch
 * Licensed under MIT License, see License file for more details
 * git clone https://github.com/marcomq/tauri-plugin-deno
**/

import { invoke } from '@tauri-apps/api/core'

/** Functions registered with `registerJs` / `registerFunction`, callable as `call.name(...)`. */
export let call: { [index: string]: (...args: any[]) => Promise<any> } = {};

/**
 * Typed view of a Deno module: every function becomes async.
 * Use with `import type * as backend from '../src-tauri/src-deno/main.ts'`.
 */
export type DenoApi<T> = {
  [K in keyof T as T[K] extends (...args: any[]) => any ? K : never]:
    T[K] extends (...args: infer A) => infer R ? (...args: A) => Promise<Awaited<R>> : never
};

async function invokeValue<T>(command: string, payload: Record<string, unknown>): Promise<T> {
  const response = await invoke<{ value: T }>(`plugin:deno|${command}`, { payload });
  return response.value;
}

/** Runs JavaScript code in the Deno backend and returns its result. Needs `deno:allow-run-code`. */
export async function runCode<T = any>(code: string): Promise<T> {
  return invokeValue<T>('run_code', { value: code });
}

/**
 * Registers a Deno function on the backend and makes it available via `call.{jsFunctionName}`.
 * Needs `deno:allow-register-function`.
 *  @param {string} denoFunctionCall - Name of the global Deno function, e.g. `greet` or `api.greet`
 *  @param {number} [numberOfArgs] - Number of arguments the function is called with; rejected if
 *    the function requires more. Use -1 or leave out to skip the check.
 *  @param {string} [jsFunctionName] - Name used in `call.jsFunctionName`. Must not contain dots.
 */
export async function registerFunction(
  denoFunctionCall: string,
  numberOfArgs?: number,
  jsFunctionName?: string): Promise<boolean> {
  if (numberOfArgs !== undefined && numberOfArgs < 0) {
    numberOfArgs = undefined;
  }
  const result = await invokeValue<boolean>('register_function', {
    functionName: denoFunctionCall,
    numberOfArgs,
  });
  registerJs(denoFunctionCall, jsFunctionName);
  return result;
}

/**
 * No server invocation - assumes that the function has already been registered in Deno,
 * e.g. via `_tauri_plugin_functions` or `export`. Makes it available as `call.{jsFunctionName}`.
 *  @param {string} denoFunctionCall - Name of the Deno function
 *  @param {string} [jsFunctionName] - Name used in `call.jsFunctionName`. Must not contain dots.
 */
export function registerJs(denoFunctionCall: string, jsFunctionName?: string): void {
  if (jsFunctionName === undefined) {
    jsFunctionName = denoFunctionCall.replaceAll(".", "_");
  }
  call[jsFunctionName] = (...args: any[]) => callFunction(denoFunctionCall, args);
}

/**
 * Calls a registered or exported Deno function. Rejects with the error message if the
 * function throws, isn't registered or the call times out.
 */
export async function callFunction<T = any>(functionName: string, args: any[] = []): Promise<T> {
  return invokeValue<T>('call_function', { functionName, args });
}

/** Reads a global variable from Deno. Needs `deno:allow-read-variable`. */
export async function readVariable<T = any>(value: string): Promise<T> {
  return invokeValue<T>('read_variable', { value });
}

/**
 * Returns a typed proxy for the functions of a Deno module:
 * ```ts
 * import type * as backend from '../src-tauri/src-deno/main.ts'
 * const api = createApi<typeof backend>()
 * const text = await api.greet('world') // typed as Promise<string>
 * ```
 * Each property access calls `callFunction(name, args)`; the function must be exported
 * or listed in `_tauri_plugin_functions`.
 */
export function createApi<T = Record<string, (...args: any[]) => any>>(): DenoApi<T> {
  return new Proxy({}, {
    get(_target, name) {
      // `then` is excluded so the proxy isn't treated as a promise when awaited.
      if (typeof name !== 'string' || name === 'then') {
        return undefined;
      }
      return (...args: any[]) => callFunction(name, args);
    },
  }) as DenoApi<T>;
}
