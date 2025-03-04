const { core } = Deno;

function argsToMessage(...args) {
  return args.map((arg) => JSON.stringify(arg)).join(" ");
}

globalThis.console = {
  log: (...args) => {
    core.print(`[out]: ${argsToMessage(...args)}\n`, false);
  },
  error: (...args) => {
    core.print(`[err]: ${argsToMessage(...args)}\n`, true);
  },
};

globalThis.fs = {
    readFile: (path) => {
        return core.ops.op_read_file(path);
    },
    readFileSync: (path) => {
        return core.ops.op_read_file_sync(path);
    },
    writeFile: (path, contents) => {
        return core.ops.op_write_file(path, contents);
    },
    writeFileSync: (path, contents) => {
        return core.ops.op_write_file_sync(path, contents);
    },
    removeFile: (path) => {
        return core.ops.op_remove_file(path);
    },
};

globalThis.req = {
  fetch: async (url) => {
    return core.ops.op_fetch(url);
  },
};

globalThis.setTimeout = async (callback, delay) => {
  core.ops.op_set_timeout(delay).then(callback);
};
