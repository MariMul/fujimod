import { For, Show, createSignal } from 'solid-js';
import { Icon } from '@fujimod/emulator-ui/ui/icons';
import type { FirmwareLibrary } from './firmware-library';
import './firmware-picker.css';

export function FirmwarePicker(props: {
  library: FirmwareLibrary;
  inputId: string;
  disabled?: boolean;
  heading?: string;
  description?: string;
}) {
  const [dragging, setDragging] = createSignal(false);
  const disabled = () => props.disabled || props.library.busy();
  return (
    <section class="base-firmware-picker" aria-label={props.heading ?? 'Base firmware'}>
      <Show when={props.heading}>
        <h3>{props.heading}</h3>
      </Show>
      <Show when={props.description}>
        <p class="firmware-description">{props.description}</p>
      </Show>
      <Show when={props.library.entries().length}>
        <div class="firmware-saved" role="radiogroup" aria-label="Firmware in this browser">
          <For each={props.library.entries()}>
            {(firmware) => (
              <div class="firmware-saved-row">
                <label>
                  <input
                    class="firmware-radio-input"
                    type="radio"
                    name={props.inputId + '-saved'}
                    aria-label={firmware.name}
                    checked={props.library.selected() === firmware.sha256}
                    disabled={disabled()}
                    onChange={() => {
                      props.library.select(firmware.sha256);
                    }}
                  />
                  <span class="firmware-radio" aria-hidden="true">
                    <Icon name="check" size={12} />
                  </span>
                  <span class="firmware-saved-text">
                    <strong>{firmware.name}</strong>
                    <span>Saved in your browser •</span>
                  </span>
                </label>
                <button
                  class="firmware-remove"
                  disabled={disabled()}
                  aria-label={'Remove ' + firmware.name}
                  onClick={() => void props.library.remove(firmware.sha256)}
                >
                  Remove
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>
      <div
        class={{
          'firmware-drop': true,
          'firmware-drop-empty': !props.library.entries().length,
          dragging: dragging(),
        }}
        onDragOver={(event) => {
          event.preventDefault();
          event.stopPropagation();
          setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(event) => {
          event.preventDefault();
          event.stopPropagation();
          setDragging(false);
          if (!disabled()) void props.library.add(event.dataTransfer?.files[0]);
        }}
      >
        <span class="firmware-add-icon">
          <Icon name="plus" size={20} />
        </span>
        <strong>{props.library.busy() ? 'Checking firmware…' : 'Add official firmware'}</strong>
        <span>
          Drop a Fujifilm .DAT file here, or{' '}
          <label class="firmware-choose" for={props.inputId}>
            choose a file
          </label>
        </span>
        <input
          class="visually-hidden"
          id={props.inputId}
          type="file"
          accept=".dat,.DAT,application/octet-stream"
          aria-label="Choose official Fujifilm firmware"
          disabled={disabled()}
          onChange={(event) => {
            void props.library.add(event.currentTarget.files?.[0]);
            event.currentTarget.value = '';
          }}
        />
      </div>
      <Show when={props.library.error()}>
        <p class="firmware-error" role="alert">
          {props.library.error()}
        </p>
      </Show>
    </section>
  );
}
