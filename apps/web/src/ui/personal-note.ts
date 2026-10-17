import cameraInventory from "../../data/cameras.json";
import featureIcons from "./assets/feature-icons.svg?url";
import type { CameraSupport } from "./cameras";

const labels: Record<string, string> = {
  aspect: "Aspect ratio",
  highlight: "Highlight metering",
  snap: "Snap focus",
  recipes: "Save recipe",
  "white-balance": "White balance",
  shift: "White balance shift",
  film: "Film simulation",
  eterna: "ETERNA",
  "classic-neg": "Classic Neg.",
  "bleach-bypass": "Bleach Bypass",
  "nostalgic-neg": "Nostalgic Neg.",
  "reala-ace": "REALA ACE",
};

function featureIcon(name: string, ratio = false) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.classList.add("feature-icon");
  if (ratio) svg.classList.add("ratio");
  const label = ratio ? name.replace("-", ":") : labels[name];
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", label);
  const title = document.createElementNS(svg.namespaceURI, "title");
  title.textContent = label;
  const use = document.createElementNS(svg.namespaceURI, "use");
  use.setAttribute("href", `${featureIcons}#${name}`);
  svg.append(title, use);
  return svg;
}

function iconList(host: HTMLElement, names: string[], ratio = false) {
  names.forEach((name, index) => {
    if (index) host.append(", ");
    host.append(featureIcon(name, ratio));
  });
}

export function personalNote(host: HTMLElement) {
  const date = new Date();
  // Compare calendar dates in the visitor's timezone; DST cannot add a day.
  const today = Date.UTC(date.getFullYear(), date.getMonth(), date.getDate());
  const age = Math.max(0, Math.floor((today - Date.UTC(2026, 9, 3)) / 86_400_000));
  host.querySelector("#project-age")!.textContent = String(age);
  host.querySelector("#camera-count")!.textContent = String(
    new Set(cameraInventory.map((camera) => camera.name)).size,
  );
  for (const slot of host.querySelectorAll<HTMLElement>("[data-feature-icon]")) {
    slot.replaceWith(featureIcon(slot.dataset.featureIcon!));
  }
  iconList(host.querySelector("#crop-ratios")!, [
    "65-24", "17-6", "3-2", "4-3", "16-9", "3-4", "1-1", "7-6", "5-4",
  ], true);
  iconList(host.querySelector("#other-films")!, [
    "classic-neg", "bleach-bypass", "nostalgic-neg", "reala-ace",
  ]);
}

export function noteCameraSupport(host: HTMLElement, cameras: CameraSupport[]) {
  const models = [...new Set(cameras.filter((camera) => camera.mod).map(
    (camera) => camera.firmware.model,
  ))];
  host.querySelector("#supported-models")!.textContent = models.length
    ? new Intl.ListFormat("en", { style: "long", type: "conjunction" }).format(models)
    : "no cameras in this build";
}
