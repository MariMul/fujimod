import type { DetentSound } from '../feedback/detent-sound';
import type { BootInput } from '../engine/api';
export interface ControlNode {
  kind: 'control' | 'grid' | 'row' | 'column' | 'divider' | 'space' | 'command' | 'dpad';
  className?: string;
  id?: string;
  variant?: 'dial' | 'ticks' | 'segment' | 'shutter' | 'key' | 'record';
  caption?: string;
  operation?: 'power';
  face?: string[];
  hint?: string;
  icon?: string;
  labels?: Record<string, string>;
  children?: ControlNode[];
  bindings?: Record<string, string>;
}
export interface PowerCombo {
  label: string;
  keys: { face?: string; icon?: string; record?: boolean }[];
  inputs: BootInput[];
}
export interface DevicePanel {
  /** Acoustic character of each physical dial, shared by panel and model. */
  detentSounds?: Record<string, DetentSound>;
  layout: ControlNode[];
  combos: PowerCombo[];
}
