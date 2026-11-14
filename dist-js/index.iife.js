if ('__TAURI__' in window) {
var __TAURI_PLUGIN_DENO_API__ = (function (exports) {
    'use strict';

    /******************************************************************************
    Copyright (c) Microsoft Corporation.

    Permission to use, copy, modify, and/or distribute this software for any
    purpose with or without fee is hereby granted.

    THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
    REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY
    AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
    INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM
    LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR
    OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR
    PERFORMANCE OF THIS SOFTWARE.
    ***************************************************************************** */
    /* global Reflect, Promise, SuppressedError, Symbol, Iterator */


    typeof SuppressedError === "function" ? SuppressedError : function (error, suppressed, message) {
        var e = new Error(message);
        return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
    };

    /**
     * Sends a message to the backend.
     * @example
     * ```typescript
     * import { invoke } from '@tauri-apps/api/core';
     * await invoke('login', { user: 'tauri', password: 'poiwe3h4r5ip3yrhtew9ty' });
     * ```
     *
     * @param cmd The command name.
     * @param args The optional arguments to pass to the command.
     * @param options The request options.
     * @return A promise resolving or rejecting to the backend response.
     *
     * @since 1.0.0
     */
    async function invoke(cmd, args = {}, options) {
        return window.__TAURI_INTERNALS__.invoke(cmd, args, options);
    }

    /** Tauri Plugin Deno
     * © Copyright 2025, by Marco Mengelkoch
     * Licensed under MIT License, see License file for more details
     * git clone https://github.com/marcomq/tauri-plugin-deno
    **/
    /** Functions registered with `registerJs` / `registerFunction`, callable as `call.name(...)`. */
    let call = {};
    async function invokeValue(command, payload) {
        const response = await invoke(`plugin:deno|${command}`, { payload });
        return response.value;
    }
    /** Runs JavaScript code in the Deno backend and returns its result. Needs `deno:allow-run-code`. */
    async function runCode(code) {
        return invokeValue('run_code', { value: code });
    }
    /**
     * Registers a Deno function on the backend and makes it available via `call.{jsFunctionName}`.
     * Needs `deno:allow-register-function`.
     *  @param {string} denoFunctionCall - Name of the global Deno function, e.g. `greet` or `api.greet`
     *  @param {number} [numberOfArgs] - Number of arguments the function is called with; rejected if
     *    the function requires more. Use -1 or leave out to skip the check.
     *  @param {string} [jsFunctionName] - Name used in `call.jsFunctionName`. Must not contain dots.
     */
    async function registerFunction(denoFunctionCall, numberOfArgs, jsFunctionName) {
        if (numberOfArgs !== undefined && numberOfArgs < 0) {
            numberOfArgs = undefined;
        }
        const result = await invokeValue('register_function', {
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
    function registerJs(denoFunctionCall, jsFunctionName) {
        if (jsFunctionName === undefined) {
            jsFunctionName = denoFunctionCall.replaceAll(".", "_");
        }
        call[jsFunctionName] = (...args) => callFunction(denoFunctionCall, args);
    }
    /**
     * Calls a registered or exported Deno function. Rejects with the error message if the
     * function throws, isn't registered or the call times out.
     */
    async function callFunction(functionName, args = []) {
        return invokeValue('call_function', { functionName, args });
    }
    /** Reads a global variable from Deno. Needs `deno:allow-read-variable`. */
    async function readVariable(value) {
        return invokeValue('read_variable', { value });
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
    function createApi() {
        return new Proxy({}, {
            get(_target, name) {
                // `then` is excluded so the proxy isn't treated as a promise when awaited.
                if (typeof name !== 'string' || name === 'then') {
                    return undefined;
                }
                return (...args) => callFunction(name, args);
            },
        });
    }

    exports.call = call;
    exports.callFunction = callFunction;
    exports.createApi = createApi;
    exports.readVariable = readVariable;
    exports.registerFunction = registerFunction;
    exports.registerJs = registerJs;
    exports.runCode = runCode;

    return exports;

})({});
Object.defineProperty(window.__TAURI__, 'deno', { value: __TAURI_PLUGIN_DENO_API__ }) }
