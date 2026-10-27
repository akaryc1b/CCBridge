import { expect, it } from "vitest";
import { maintenanceLabel } from "./runnerMaintenance";
it("uses only current connection status and distinguishes idle/error/newer", () => {
  const c = {
    connected: true,
    connection_id: "new",
    probe: { runner_version: "1.0.0" },
  };
  expect(
    maintenanceLabel(
      { ...c, runner_maintenance: { connection_id: "old", state: "ready" } },
      "1.0.1",
    ),
  ).toBe("等待自动检查");
  for (const [state, label] of [
    ["waiting_idle", "空闲后自动更新"],
    ["error", "检查或更新失败"],
    ["newer", "服务端较新，请更新 App"],
  ])
    expect(
      maintenanceLabel(
        { ...c, runner_maintenance: { connection_id: "new", state } },
        "1.0.1",
      ),
    ).toBe(label);
  expect(maintenanceLabel({ ...c, connected: false }, "1.0.0")).toBe("未连接");
});
