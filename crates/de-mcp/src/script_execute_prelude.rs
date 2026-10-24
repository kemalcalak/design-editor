//! The JavaScript prelude of the `execute` script tool: pen.dev's `execute`
//! API (Insert / Copy / Update / Replace / Move / Delete / Generate /
//! SetVariables, Get with visitors, GetVariables, FindEmptySpace, Print,
//! TakeScreenshot, Export) over the one host function `__host(op, json)`.
//!
//! Every host call answers `{"ok": value}` or `{"err": message}`; an error
//! becomes a thrown `Error`, so an uncaught one fails the run and nothing is
//! applied. The visitor walk runs here, over the tree the host returns for
//! one `Get` call (`script_execute_tree.rs`), so a visitor can call writes
//! without re-entering the host mid-walk.

pub(super) const EXECUTE_PRELUDE: &str = r##"
(function () {
  var host = globalThis.__host;
  delete globalThis.__host;
  function call(op, args) {
    var reply = JSON.parse(host(op, JSON.stringify(args === undefined ? null : args)));
    if (reply.err !== undefined) throw new Error(reply.err);
    return reply.ok;
  }
  function target(value, label) {
    if (value !== null && typeof value === "object") {
      throw new Error(label + " must be an id string: pass node.id, not the node object");
    }
    return value === undefined ? undefined : value;
  }
  function ids(value, label) {
    if (typeof value === "string") return [value];
    if (Array.isArray(value)) return value.map(function (v) { return target(v, label); });
    throw new Error(label + " takes an id string or an array of id strings");
  }
  function text(value) {
    if (typeof value === "string") return value;
    if (value === undefined) return "undefined";
    var json = JSON.stringify(value);
    return json === undefined ? String(value) : json;
  }
  Object.defineProperty(globalThis, "document", { value: "document", writable: false });
  globalThis.Insert = function (parent, nodeData) {
    return call("insert", { parent: target(parent, "Insert parent"), node: nodeData });
  };
  globalThis.Copy = function (path, parent, copyNodeData) {
    return call("copy", { path: target(path, "Copy path"), parent: target(parent, "Copy parent"), data: copyNodeData });
  };
  globalThis.Update = function (path, updateData) {
    call("update", { path: target(path, "Update path"), data: updateData });
  };
  globalThis.Replace = function (path, nodeData) {
    return call("replace", { path: target(path, "Replace path"), data: nodeData });
  };
  globalThis.Move = function (path, parent, index) {
    call("move", { path: target(path, "Move path"), parent: target(parent, "Move parent"), index: index });
  };
  globalThis.Delete = function (path) {
    call("delete", { path: target(path, "Delete path") });
  };
  globalThis.Generate = function (type, nodeId, prompt) {
    return call("generate", { type: type, nodeId: target(nodeId, "Generate nodeId"), prompt: prompt });
  };
  globalThis.SetVariables = function (variables, replace) {
    call("setVariables", { variables: variables, replace: replace === true });
  };
  globalThis.GetVariables = function () {
    return call("getVariables", null);
  };
  globalThis.FindEmptySpace = function (input) {
    return call("findEmptySpace", input || {});
  };
  globalThis.Print = function () {
    var parts = [];
    for (var i = 0; i < arguments.length; i++) parts.push(text(arguments[i]));
    call("print", { line: parts.join(" ") });
  };
  globalThis.TakeScreenshot = function (nodeIds) {
    call("screenshot", { nodeIds: ids(nodeIds, "TakeScreenshot") });
  };
  globalThis.Export = function (nodeIds, format, outputPath, options) {
    call("export", { nodeIds: ids(nodeIds, "Export"), format: format, outputPath: outputPath, options: options || {} });
  };
  globalThis.Get = function (a, b, c) {
    var path, visit, options;
    if (typeof a === "function") {
      visit = a;
      options = b;
    } else {
      path = target(a, "Get path");
      if (typeof b === "function") {
        visit = b;
        options = c;
      } else {
        options = b;
      }
    }
    options = options || {};
    if (visit === undefined) {
      if (path === undefined || path === null || path === "document") {
        throw new Error("Get needs a node id, or a visitor to walk the document: Get(visit) / Get(id, visit)");
      }
      return call("get", { path: path, options: options });
    }
    var tree = call("tree", { path: path === "document" ? null : path, options: options });
    var out = [];
    function walk(entries, parentCtx, depth) {
      for (var i = 0; i < entries.length; i++) {
        var entry = entries[i];
        var skip = false;
        var ctx = {
          node: entry.n,
          parentCtx: parentCtx,
          depth: depth,
          index: entry.i,
          bounds: entry.b,
          skipChildren: function () { skip = true; }
        };
        if (entry.p !== undefined) ctx.problems = entry.p;
        var result = visit(entry.n, ctx);
        if (result !== undefined) out.push(result);
        if (!skip && entry.c) walk(entry.c, ctx, depth + 1);
      }
    }
    walk(tree, undefined, 0);
    return out;
  };
  var log = function () { globalThis.Print.apply(null, arguments); };
  globalThis.console = { log: log, info: log, warn: log, error: log, debug: log };
})();
"##;
