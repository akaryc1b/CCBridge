export function maintenanceLabel(
  connection: Record<string, any>,
  version: string,
) {
  if (!connection.connected) return "未连接";
  const status = connection.runner_maintenance;
  if (status?.connection_id === connection.connection_id) {
    const labels: Record<string, string> = {
      checking: "检查中…",
      updating: "更新中…",
      waiting_idle: "空闲后自动更新",
      ready: "已就绪",
      newer: "服务端较新，请更新 App",
      error: "检查或更新失败",
      action_required: "需手动更新",
    };
    if (labels[status.state]) return labels[status.state];
  }
  return connection.probe?.runner_version === version
    ? "已就绪"
    : "等待自动检查";
}
