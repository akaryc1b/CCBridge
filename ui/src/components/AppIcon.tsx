// Use the exact asset included in the native app bundle; do not substitute a UI glyph.
const iconUrl = new URL("../../../src-tauri/icons/128x128.png", import.meta.url)
  .href;

export function AppIcon({ size = 24 }: { size?: number }) {
  return (
    <img
      className="app-icon"
      src={iconUrl}
      width={size}
      height={size}
      alt=""
      aria-hidden="true"
      draggable={false}
    />
  );
}
