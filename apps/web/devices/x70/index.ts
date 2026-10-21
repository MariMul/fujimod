export { credit as previewCredit } from './parts.json';
import panelData from './panel.json';
import type { DevicePanel } from '@fujimod/emulator-ui/ui/layout';
export const panel = panelData as DevicePanel;
export const preview = new URL('./preview.png', import.meta.url).href;

export const sdBoot = true;
