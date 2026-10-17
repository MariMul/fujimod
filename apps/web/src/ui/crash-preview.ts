import dunes from "../engine/assets/default-dunes.png";
import type { EngineFault } from "../engine/fault";
import type { ModelViewer } from "../viewer/model";

/** Development-only fixture, loaded by main's guarded URL handler. */
export async function showCrashPreview(
  viewer: ModelViewer,
  showFault: (fault: EngineFault) => void,
) {
  const image = new Image();
  image.src = dunes;
  await image.decode();
  const canvas = document.createElement("canvas");
  canvas.width = 720;
  canvas.height = 480;
  const context = canvas.getContext("2d")!;
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  const rgba = context.getImageData(0, 0, canvas.width, canvas.height).data;
  const rgb = new Uint8Array(canvas.width * canvas.height * 3);
  for (let s = 0, t = 0; s < rgba.length; s += 4, t += 3)
    rgb.set(rgba.subarray(s, s + 3), t);
  viewer.frame({ rgb, width: canvas.width, height: canvas.height, sequence: 0 });
  viewer.view("lcd");
  document.getElementById("workspace")!.classList.remove("idle");
  for (const id of ["firmware-prompt", "idle-cameras", "panel", "view-3d"])
    document.getElementById(id)!.hidden = true;
  document.getElementById("viewer-bar")!.hidden = false;
  document.getElementById("camera-title")!.textContent = "Crash overlay preview";
  document.getElementById("view-lcd")!.setAttribute("aria-pressed", "true");
  document.getElementById("view-3d")!.setAttribute("aria-pressed", "false");
  showFault({
    kind: "exception",
    message: "Development preview: the emulator has stopped.",
    details: { type: "CrashPreview", message: "Requested by ?preview=crash" },
  });
  // Keep the fixture focused on the LCD artwork rather than covering it with diagnostics.
  document.getElementById("engine-status")!.hidden = true;
  document.getElementById("workspace")!.scrollIntoView({ block: "start" });
}
