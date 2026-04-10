import { invoke } from "@tauri-apps/api/core";

interface ExportOptions {
  openAfterSave?: boolean;
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => {
      reject(reader.error ?? new Error("Failed to read export data."));
    };
    reader.onloadend = () => {
      const result = reader.result;
      if (typeof result !== "string") {
        reject(new Error("Failed to encode export data."));
        return;
      }
      const marker = result.indexOf(",");
      resolve(marker >= 0 ? result.slice(marker + 1) : result);
    };
    reader.readAsDataURL(blob);
  });
}

function triggerBrowserDownload(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.style.display = "none";
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  window.setTimeout(() => URL.revokeObjectURL(url), 10000);
}

export async function exportBlobWithTauriFallback(
  blob: Blob,
  filename: string,
  options: ExportOptions = {}
): Promise<string | null> {
  if (!(blob instanceof Blob) || blob.size === 0) {
    throw new Error("Generated export file is empty.");
  }

  try {
    const base64Data = await blobToBase64(blob);
    const savedPath = await invoke<string>("save_export_file", {
      filename,
      base64Data,
      openAfterSave: options.openAfterSave ?? false,
    });
    if (typeof savedPath === "string" && savedPath.trim().length > 0) {
      return savedPath.trim();
    }
  } catch (error) {
    console.warn("Tauri export save failed, using browser download fallback", error);
  }

  triggerBrowserDownload(blob, filename);
  return null;
}

export async function exportTextWithTauriFallback(
  text: string,
  filename: string,
  mimeType = "text/plain;charset=utf-8;",
  options: ExportOptions = {}
): Promise<string | null> {
  const blob = new Blob([text], { type: mimeType });
  return exportBlobWithTauriFallback(blob, filename, options);
}

export async function revealExportedFileInFolder(path: string): Promise<void> {
  const target = path.trim();
  if (!target) {
    throw new Error("Export path is empty.");
  }
  await invoke("reveal_export_file", { path: target });
}
