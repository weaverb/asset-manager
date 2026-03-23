import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";

export { isTauri };

/**
 * Invoke a Rust command. Only works inside the Tauri webview (`npm run tauri dev`).
 * A plain browser tab at http://localhost:1420 does not have the IPC bridge.
 */
export function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauri()) {
    return Promise.reject(
      new Error(
        "Tauri IPC is not available. Run the desktop app with: npm run tauri dev",
      ),
    );
  }
  return tauriInvoke<T>(cmd, args);
}
