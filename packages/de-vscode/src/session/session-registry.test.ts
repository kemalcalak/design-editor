import { test, expect } from "bun:test";
import { SessionRegistry } from "./session-registry";
import type { PenSession } from "./pen-session";

function fakeSession(id: string): PenSession {
  return { id } as unknown as PenSession;
}

test("register + setActive + activeSession returns the active one", () => {
  const r = new SessionRegistry();
  const a = fakeSession("a");
  const b = fakeSession("b");
  r.register("/a.de", a);
  r.register("/b.de", b);
  r.setActive("/a.de");
  expect(r.activeSession()).toBe(a);
  r.setActive("/b.de");
  expect(r.activeSession()).toBe(b);
});

test("setActive(undefined) clears the active pointer", () => {
  const r = new SessionRegistry();
  r.register("/a.de", fakeSession("a"));
  r.setActive("/a.de");
  r.setActive(undefined);
  expect(r.activeSession()).toBeUndefined();
});

test("setActive to an unregistered file is ignored", () => {
  const r = new SessionRegistry();
  r.register("/a.de", fakeSession("a"));
  r.setActive("/a.de");
  r.setActive("/never.de");
  // unchanged — still /a.de
  expect((r.activeSession() as unknown as { id: string }).id).toBe("a");
});

test("unregister clears active if it was the active file", () => {
  const r = new SessionRegistry();
  r.register("/a.de", fakeSession("a"));
  r.setActive("/a.de");
  r.unregister("/a.de");
  expect(r.activeSession()).toBeUndefined();
});

test("unregister a non-active file leaves active intact", () => {
  const r = new SessionRegistry();
  const a = fakeSession("a");
  r.register("/a.de", a);
  r.register("/b.de", fakeSession("b"));
  r.setActive("/a.de");
  r.unregister("/b.de");
  expect(r.activeSession()).toBe(a);
});
