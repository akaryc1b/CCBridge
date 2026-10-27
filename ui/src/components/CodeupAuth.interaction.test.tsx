// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CodeupAuth } from "./CodeupAuth";
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) =>
    command === "git_vps_auth_status"
      ? { configured: true, username: "fixture" }
      : { configured: false, helper_configured: true },
  ),
}));
beforeEach(() => vi.mocked(invoke).mockClear());
afterEach(cleanup);
it("can clear and retype repository URL and close after repeated openings", async () => {
  function Harness() {
    const [open, setOpen] = useState(true);
    return (
      <>
        <button onClick={() => setOpen(true)}>Open</button>
        <CodeupAuth
          open={open}
          onOpenChange={setOpen}
          connected
          account="fixture@localhost:22"
          url="https://codeup.aliyun.com/example/repo.git"
          initialScope="remote"
        />
      </>
    );
  }
  render(<Harness />);
  for (let i = 0; i < 3; i++) {
    const input = (await screen.findByLabelText(
      "仓库访问",
    )) as HTMLInputElement;
    await waitFor(() => expect(input.disabled).toBe(false));
    fireEvent.change(input, { target: { value: "" } });
    expect(input.value).toBe("");
    fireEvent.change(input, {
      target: { value: "https://codeup.aliyun.com/fixture/repo.git" },
    });
    expect(input.value).toContain("fixture/repo.git");
    const close = screen.getAllByRole("button", { name: "关闭" })[0];
    fireEvent.click(close);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
  }
});

it("shows network failures without offering to replace a valid credential", async () => {
  const network =
    "远端 Codeup 访问失败：远端无法连接 Git 服务，请检查网络后重试";
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "git_vps_auth_test") throw new Error(network);
    return command === "git_vps_auth_status"
      ? { configured: true, username: "fixture" }
      : { helper_configured: true };
  });
  render(
    <CodeupAuth
      open
      onOpenChange={() => {}}
      connected
      account="fixture@localhost:22"
      url="https://codeup.aliyun.com/example/repo.git"
      initialScope="remote"
    />,
  );
  await waitFor(() =>
    expect(
      (screen.getByRole("button", { name: "检测" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false),
  );
  fireEvent.click(screen.getByRole("button", { name: "检测" }));
  await screen.findByText(network);
  expect(screen.queryByRole("button", { name: "更换认证" })).toBeNull();
});

it("keeps one dialog portal while its parent refreshes", async () => {
  const view = render(
    <CodeupAuth
      open
      connected
      account="fixture@localhost:22"
      url=""
      initialScope="remote"
      onOpenChange={() => {}}
      onActivity={() => {}}
    />,
  );
  await waitFor(() => expect(screen.getByRole("dialog")).toBeTruthy());
  for (let i = 0; i < 12; i++) {
    view.rerender(
      <CodeupAuth
        open
        connected
        account="fixture@localhost:22"
        url=""
        initialScope="remote"
        onOpenChange={() => {}}
        onActivity={() => {}}
      />,
    );
  }
  expect(
    document.querySelectorAll('[data-slot="dialog-content"]'),
  ).toHaveLength(1);
  expect(document.querySelectorAll("#codeup-url")).toHaveLength(1);
});

it("removes old portals when the connection key changes", async () => {
  const view = render(
    <CodeupAuth
      key="connection-0"
      open
      connected
      account="fixture@localhost:22"
      url=""
      initialScope="remote"
      onOpenChange={() => {}}
    />,
  );
  await screen.findByRole("dialog");
  for (let i = 1; i < 8; i++) {
    view.rerender(
      <CodeupAuth
        key={`connection-${i}`}
        open
        connected
        account="fixture@localhost:22"
        url=""
        initialScope="remote"
        onOpenChange={() => {}}
      />,
    );
    await waitFor(() =>
      expect(
        document.querySelectorAll('[data-slot="dialog-content"]'),
      ).toHaveLength(1),
    );
  }
});

it("does not save or silently fall back when testing a rejected draft credential", async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "git_access")
      throw new Error("本机 Git 未通过认证或仓库读取校验");
    return command === "git_auth_status"
      ? { configured: false, helper_configured: false }
      : {};
  });
  render(
    <CodeupAuth
      open
      connected
      account="fixture@localhost:22"
      url="https://codeup.aliyun.com/example/repo.git"
      initialScope="local"
      onOpenChange={() => {}}
    />,
  );
  await screen.findByLabelText("用户名");
  fireEvent.change(screen.getByLabelText("用户名"), {
    target: { value: "synthetic-rejected-user" },
  });
  fireEvent.change(screen.getByLabelText("密码或令牌"), {
    target: { value: "not-a-real-token" },
  });
  fireEvent.click(screen.getByRole("button", { name: "检测" }));
  await screen.findByText("本机 Git 未通过认证或仓库读取校验");
  const calls = vi.mocked(invoke).mock.calls;
  expect(calls.filter(([command]) => command === "git_access")).toHaveLength(1);
  expect(
    calls.some(([command]) =>
      ["git_auth_save", "git_vps_auth_install", "git_vps_auth_test"].includes(
        command,
      ),
    ),
  ).toBe(false);
  expect(
    calls.find(([command]) => command === "git_access")?.[1],
  ).toMatchObject({
    request: {
      username: "synthetic-rejected-user",
      secret: "not-a-real-token",
    },
  });
});
