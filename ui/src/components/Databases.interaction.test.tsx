// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { Databases } from "./Databases";
afterEach(cleanup);
function mount() {
  const request = vi.fn(async (command: string) => {
    if (command === "database_list") return { connections: [], bindings: [] };
    if (command === "database_detect")
      return { node: "/fixture/node", package_root: "/fixture/package" };
    if (command === "database_bind") throw new Error("映射别名无效");
    if (command === "database_save") throw new Error("连接字段无效");
    return {};
  });
  render(
    <Databases
      connected
      projects={[{ name: "Alpha", path: "/fixture/alpha" }]}
      currentProject="/fixture/alpha"
      request={request}
      refreshProjects={async () => {}}
    />,
  );
  return request;
}
describe("database error context", () => {
  it("does not carry a mapping error into connections or a new form", async () => {
    mount();
    await waitFor(() =>
      expect(
        (screen.getByRole("button", { name: "新增连接" }) as HTMLButtonElement)
          .disabled,
      ).toBe(false),
    );
    fireEvent.click(screen.getByRole("button", { name: "项目映射" }));
    fireEvent.click(screen.getByRole("button", { name: "保存项目映射" }));
    await screen.findByText("映射别名无效");
    fireEvent.click(screen.getByRole("button", { name: "连接" }));
    expect(screen.queryByText("映射别名无效")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "新增连接" }));
    expect(screen.queryByText("映射别名无效")).toBeNull();
  });
  it("renders a failed form submission once, inside its dialog", async () => {
    mount();
    await waitFor(() =>
      expect(
        (screen.getByRole("button", { name: "新增连接" }) as HTMLButtonElement)
          .disabled,
      ).toBe(false),
    );
    fireEvent.click(screen.getByRole("button", { name: "新增连接" }));
    for (const [label, value] of [
      ["连接名称", "Fixture"],
      ["数据库地址", "127.0.0.1"],
      ["用户名", "fixture"],
      ["数据库名", "test"],
    ])
      fireEvent.change(screen.getByLabelText(label), { target: { value } });
    fireEvent.click(screen.getByRole("button", { name: "保存连接" }));
    await waitFor(() =>
      expect(screen.getAllByText("连接字段无效")).toHaveLength(1),
    );
    expect(
      screen.getByRole("dialog").contains(screen.getByText("连接字段无效")),
    ).toBe(true);
  });
});
