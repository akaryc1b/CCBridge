import { expect, it } from "vitest";
import {
  normalizeRemotePath,
  remoteContains,
  remoteBasename,
  remoteParent,
} from "./remotePath";
it("uses Windows path boundaries independently of the Mac host", () => {
  expect(normalizeRemotePath("\\\\?\\C:\\Users\\User\\项目")).toBe(
    "c:/users/user/项目",
  );
  expect(remoteContains("C:\\Users\\User", "c:/Users/USER/项目")).toBe(true);
  expect(remoteContains("C:\\Users\\User", "C:\\Users\\UserOther")).toBe(false);
  expect(remoteBasename("C:\\Users\\User\\项目")).toBe("项目");
  expect(remoteParent("C:\\Users\\User\\项目", "C:\\Users\\User")).toBe(
    "C:\\Users\\User",
  );
  expect(remoteParent("C:\\Users\\User", "C:\\Users\\User")).toBe(
    "C:\\Users\\User",
  );
});
it("preserves Unix case sensitivity and user boundary", () => {
  expect(remoteContains("/home/u", "/home/U/file")).toBe(false);
  expect(remoteParent("/home/u", "/home/u")).toBe("/home/u");
  expect(remoteParent("/home/u/project", "/home/u")).toBe("/home/u");
});
