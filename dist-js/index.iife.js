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
    let call = {}; // array of functions
    async function runCode(code) {
        return await invoke('plugin:deno|run_code', {
            payload: {
                value: code,
            },
        }).then((r) => {
            return r.value;
        });
    }
    /**
     * Registers function on server and makes it available via `call.{jsFunctionName}`
     *  @param {string} denoFunctionCall - The deno js function call
     *  @param {number} [numberOfArgs] - Number of arguments, used for validation in deno, use -1 to ignore this value
     *  @param {string} [jsFunctionName] - Name that is used in javscript: "call.jsFunctionName". Must not contain dots.
     */
    async function registerFunction(denoFunctionCall, numberOfArgs, jsFunctionName) {
        if (numberOfArgs !== undefined && numberOfArgs < 0) {
            numberOfArgs = undefined;
        }
        return await invoke('plugin:deno|register_function', {
            payload: {
                denoFunctionCall,
                numberOfArgs
            },
        }).then((r) => {
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
    async function registerJs(denoFunctionCall, jsFunctionName) {
        if (jsFunctionName === undefined) {
            jsFunctionName = denoFunctionCall.replaceAll(".", "_");
        }
        call[jsFunctionName] = function (...args) { return callFunction(denoFunctionCall, args); };
    }
    /**
     * Calling previously registered function
     */
    async function callFunction(functionName, args) {
        return invoke('plugin:deno|call_function', {
            payload: {
                functionName,
                args,
            },
        }).then((r) => {
            return r.value;
        });
    }
    /**
     * Read variable name from deno
     */
    async function readVariable(value) {
        return invoke('plugin:deno|read_variable', {
            payload: {
                value,
            },
        }).then((r) => {
            return r.value;
        });
    }

    exports.call = call;
    exports.callFunction = callFunction;
    exports.readVariable = readVariable;
    exports.registerFunction = registerFunction;
    exports.registerJs = registerJs;
    exports.runCode = runCode;

    return exports;

})({});
Object.defineProperty(window.__TAURI__, 'deno', { value: __TAURI_PLUGIN_DENO_API__ }) }
