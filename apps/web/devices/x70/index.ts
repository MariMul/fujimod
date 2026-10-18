import parts from './parts.json';
import panelData from './panel.json';
import type { DevicePanel } from '../../src/ui/layout';
export const panel = panelData as DevicePanel;
export const preview = new URL('./preview.png', import.meta.url).href;
export const initialPositions = { lcd_tilt: 'normal' };
export const model = { ...parts, url: 'devices/x70/x70.glb' };
