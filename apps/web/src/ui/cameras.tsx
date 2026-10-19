import { For } from 'solid-js';
import type { Firmware } from '../catalog';
import { Icon } from './icons';
export interface Generation {
  name: string;
  years: string;
  models: string[];
}
export interface CameraSupport {
  firmware: Firmware;
  mod: boolean;
  emu: boolean;
}
function Chips(props: { camera: CameraSupport }) {
  return (
    <>
      <For each={props.camera.mod ? [true] : []}>
        {() => (
          <span class="chip mod">
            <img src="brand.png" alt="Fujimod" />
          </span>
        )}
      </For>
      <For each={props.camera.emu ? [true] : []}>{() => <span class="chip emu">EMU</span>}</For>
    </>
  );
}
function FirmwareLink(props: { camera: CameraSupport }) {
  return (
    <a
      class="getfw"
      href={props.camera.firmware.download_url}
      target="_blank"
      rel="noopener noreferrer"
    >
      Firmware
      <Icon name="external" size={11} />
    </a>
  );
}
function EmulateButton(props: { camera: CameraSupport; emulate: (firmware: Firmware) => void }) {
  return (
    <For each={props.camera.emu ? [true] : []}>
      {() => (
        <button
          class="getfw"
          type="button"
          aria-label={`Try ${props.camera.firmware.model} emulator`}
          onClick={() => props.emulate(props.camera.firmware)}
        >
          Try emulator
        </button>
      )}
    </For>
  );
}
export function CameraCard(props: {
  camera: CameraSupport;
  emulate: (firmware: Firmware) => void;
}) {
  return (
    <div class="body">
      <span class="nm">{props.camera.firmware.model}</span>
      <Chips camera={props.camera} />
      <FirmwareLink camera={props.camera} />
      <EmulateButton camera={props.camera} emulate={props.emulate} />
    </div>
  );
}
function GenerationGroup(props: {
  group: Generation;
  cameras: CameraSupport[];
  emulate: (firmware: Firmware) => void;
}) {
  const supported = () =>
    props.cameras.filter((camera) => props.group.models.includes(camera.firmware.model));
  const remaining = () =>
    props.group.models.filter(
      (name) => !supported().some((camera) => camera.firmware.model === name),
    );
  return (
    <div class="gen">
      <h4>
        {props.group.name}
        <span>{props.group.years}</span>
      </h4>
      <For each={supported()}>
        {(camera) => (
          <div class="cam">
            <div class="n">
              {camera.firmware.model}
              <Chips camera={camera} />
            </div>
            <FirmwareLink camera={camera} />{' '}
            <EmulateButton camera={camera} emulate={props.emulate} />
          </div>
        )}
      </For>
      <For each={remaining().length ? [true] : []}>
        {() => (
          <div class="rest">
            <b>NOT SUPPORTED YET</b>
            {remaining().join(' · ')}
          </div>
        )}
      </For>
    </div>
  );
}
export function CameraTable(props: {
  columns: Generation[][];
  cameras: CameraSupport[];
  emulate: (firmware: Firmware) => void;
}) {
  return (
    <section class="gens" id="camera-table" aria-label="Cameras">
      <For each={props.columns}>
        {(groups) => (
          <div class="col">
            <For each={groups}>
              {(group) => (
                <GenerationGroup group={group} cameras={props.cameras} emulate={props.emulate} />
              )}
            </For>
          </div>
        )}
      </For>
    </section>
  );
}
