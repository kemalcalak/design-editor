import { describe, expect, test } from "bun:test";
import { encodeFigBytes, figSaveTargetPath, isFigPath } from "./fig-source";

describe("fig source helpers", () => {
  test("detects .fig case-insensitively", () => {
    expect(isFigPath("/a/b/design.fig")).toBe(true);
    expect(isFigPath("/a/b/design.FIG")).toBe(true);
    expect(isFigPath("/a/b/design.de")).toBe(false);
    expect(isFigPath("/a/b/fig")).toBe(false);
  });
  test("save target is the sibling .de", () => {
    expect(figSaveTargetPath("/a/b/design.fig")).toBe("/a/b/design.de");
    expect(figSaveTargetPath("/a/b/design.FIG")).toBe("/a/b/design.de");
  });
  test("encodes bytes as base64", () => {
    expect(encodeFigBytes(new Uint8Array([102, 105, 103]))).toBe("Zmln");
  });
});
