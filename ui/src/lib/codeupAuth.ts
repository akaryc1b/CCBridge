export type AuthStatus = {
  configured?: boolean;
  storage?: string;
  username?: string;
  helper_configured?: boolean;
  loading?: boolean;
  error?: string;
  files_present?: boolean;
  entry_present?: boolean;
  included?: boolean;
};
export function authLabel(scope: string, app: AuthStatus, vps: AuthStatus) {
  if (scope === "remote") {
    if (vps.loading) return "读取中…";
    if (vps.error) return "读取失败";
    if (vps.configured)
      return `已同步${vps.username ? ` · ${vps.username}` : ""}`;
    if (vps.files_present && !vps.entry_present) return "凭据已失效";
    if (vps.entry_present && !vps.included) return "Git 配置未接入";
    if (!app.loading && !app.error && app.configured)
      return `App 服务器档 · ${app.username || ""}${app.storage !== "file" ? " · 需迁移" : ""}`;
    return "未同步";
  }
  if (app.loading) return "读取中…";
  if (app.error) return "读取失败";
  if (app.configured)
    return `App 本机认证${app.username ? ` · ${app.username}` : ""}${app.storage !== "file" ? " · 需迁移" : ""}`;
  return app.helper_configured ? "本机 Git 助手" : "未配置";
}
// Explicit source selection is independent of the currently visible pane.
export function syncRequest(
  source: string,
  account: string,
  url: string,
  username = "",
  secret = "",
) {
  return {
    scope: source === "remote_app" ? "remote" : "local",
    credential_source:
      source === "remote_app" || source === "local_app" ? "app" : source,
    vps_account: account,
    url,
    username: source === "input" ? username : "",
    secret: source === "input" ? secret : "",
  };
}
export function accessOperation(
  scope: string,
  vps: AuthStatus,
  draft: boolean,
) {
  return scope === "remote" && !draft && !vps.configured
    ? "git_access"
    : scope === "remote" && !draft
      ? "git_vps_auth_test"
      : "git_access";
}
export function sourceLabel(source: string) {
  return source === "input"
    ? "本次填写的认证"
    : source === "local_app"
      ? "App 本机认证"
      : source === "remote_app"
        ? "App 服务器档"
        : "本机 Git 凭据助手";
}
