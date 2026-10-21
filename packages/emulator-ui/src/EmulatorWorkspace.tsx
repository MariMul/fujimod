import type { JSX } from '@solidjs/web';
import { Show, createSignal } from 'solid-js';
import type { SessionState } from './session/state';
import type { SessionController } from './session/controller';
import { ViewerOverlays } from './viewer/Overlays';
import { LoadingAssets } from './LoadingAssets';
import { Controls } from './ui/controls';
import { CardBrowser } from './ui/card';
import { Connections } from './ui/connections';
import './style.css';
import './loading.css';

export function EmulatorWorkspace(props: {
  state: SessionState;
  runtime?: SessionController;
  bodyPreview: string;
  labelledBy: string;
  workspace(element: HTMLElement): void;
  viewer(element: HTMLDivElement): void;
  lcd(element: HTMLCanvasElement): void;
  selectFile(file?: File): void;
  toolbar: JSX.Element;
  children: JSX.Element;
}) {
  const [dragging, setDragging] = createSignal(false);
  return (
    <section
      ref={(element) => props.workspace(element)}
      class={['stage', { idle: props.state.idle }]}
      id="workspace"
      aria-labelledby={props.labelledBy}
    >
      <div
        ref={(element) => props.viewer(element)}
        class={['viewer lcd-mode', { dragging: dragging() }]}
        id="viewer"
        aria-label="Camera viewer"
        style={{ '--body-preview': props.bodyPreview }}
        onDragOver={(event) => {
          event.preventDefault();
          setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(event) => {
          event.preventDefault();
          setDragging(false);
          props.selectFile(event.dataTransfer?.files[0]);
        }}
      >
        <canvas
          ref={(element) => props.lcd(element)}
          id="lcd-source"
          class="flat"
          aria-label="Camera LCD"
          width="720"
          height="480"
        />
        <ViewerOverlays viewer={props.runtime?.viewer} />
        <div class="ghost" aria-hidden="true" />
        {props.toolbar}
        {props.children}
        <div id="camera-loading" class="camera-loading" hidden={!props.state.loading}>
          <Show
            when={
              !props.state.loadingAssets.some(
                (asset) => asset.status === 'loading' || asset.status === 'waiting',
              )
            }
          >
            <p>{props.state.message}</p>
          </Show>
          <LoadingAssets assets={props.state.loadingAssets} />
          <progress
            id="boot-progress"
            hidden={props.state.loadingAssets.some(
              (asset) => asset.status === 'loading' || asset.status === 'waiting',
            )}
            max="100"
            value={props.state.progress}
            aria-label={props.state.message || 'Loading camera'}
          />
        </div>
        <span id="status" class={{ error: props.state.error }} role="status" aria-live="polite">
          {props.state.message}
        </span>
        <span id="live-label">{props.state.live}</span>
      </div>
      <aside
        class="panel"
        id="panel"
        aria-label="Camera controls"
        hidden={!props.state.panelVisible}
      >
        <div id="controls">
          <Controls session={props.runtime?.panel} />
        </div>
        <hr />
        <CardBrowser
          engine={props.runtime?.cardEngine()}
          present={(value) => props.runtime?.insertCard(value) ?? Promise.resolve()}
          format={() => props.runtime?.formatCard() ?? Promise.resolve()}
        />
        <Connections rows={props.state.connections} />
      </aside>
    </section>
  );
}
