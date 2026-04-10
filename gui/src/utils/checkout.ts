import { invoke } from "@tauri-apps/api/core";

export async function openCheckoutUrl(checkoutUrl: string): Promise<void> {
  const url = checkoutUrl.trim();
  if (!url) return;
  const isDesktopTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

  if (isDesktopTauri) {
    try {
      await invoke("open_external_url", { url });
      return;
    } catch {
      const popup = window.open(url, "_blank");
      if (popup) return;
      throw new Error("Unable to open checkout externally. Please allow popups and try again.");
    }
  }

  const popup = window.open(url, "_blank");
  if (!popup) {
    window.location.assign(url);
  }
}
