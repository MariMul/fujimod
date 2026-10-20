import { createEffect, createRoot, createSignal } from 'solid-js';
import type { ShareStatus } from '../../../../crates/usb-proxy/web/peer.ts';
import { CameraTarget } from './camera';
import { ConnectionState, type DevToolsController } from './controller';
import { Level } from './log';
import { createOwnerCountdown, type Countdown } from './owner-countdown';
import { ShareHost, newTicket, shareUrl, signalUrl } from './sharing';

/** Firmware battery level names at which the battery counts as exhausted. */
const EXHAUSTED = new Set(['PREEND', 'END']);

export enum ProblemKind {
  Battery = 'battery',
  Unplugged = 'unplugged',
  DifferentCamera = 'different camera',
  Attention = 'attention',
}
export interface Problem {
  kind: ProblemKind;
  message: string;
  /** Urgent problems shake and chime; an exhausted battery is expected. */
  urgent: boolean;
  /** The camera can be reconnected from the alert. */
  reconnect: boolean;
}
export interface Registration {
  url: string;
  camera: { model: string | null; serial: string | null };
  status: { online: boolean; reason?: string; battery?: string; battery_at_ms?: number };
}

/** Browser services the session uses; tests substitute them. */
export interface SessionEnvironment {
  location(): string;
  /** Called inside a click (Share, Connect): browsers only allow audio after one. */
  unlockAudio(): void;
  chime(): void;
  wakeLock(): Promise<{ release(): Promise<void> } | undefined>;
  visible(): boolean;
  onVisible(listener: () => void): () => void;
  onLeave(listener: () => void): () => void;
}

export function browserEnvironment(): SessionEnvironment {
  let audio: AudioContext | undefined;
  return {
    location: () => location.href,
    unlockAudio() {
      if (typeof speechSynthesis !== 'undefined') {
        const utterance = new SpeechSynthesisUtterance(' ');
        utterance.volume = 0;
        speechSynthesis.speak(utterance);
      }
      if (typeof AudioContext === 'undefined') return;
      audio ??= new AudioContext();
      void audio.resume();
    },
    chime() {
      if (!audio || audio.state !== 'running') return;
      const now = audio.currentTime;
      for (const [offset, frequency] of [
        [0, 880],
        [0.18, 1175],
      ]) {
        const tone = audio.createOscillator();
        const gain = audio.createGain();
        tone.frequency.value = frequency;
        gain.gain.setValueAtTime(0.0001, now + offset);
        gain.gain.exponentialRampToValueAtTime(0.25, now + offset + 0.02);
        gain.gain.exponentialRampToValueAtTime(0.0001, now + offset + 0.35);
        tone.connect(gain).connect(audio.destination);
        tone.start(now + offset);
        tone.stop(now + offset + 0.4);
      }
    },
    async wakeLock() {
      const lock = (
        navigator as Navigator & {
          wakeLock?: { request(type: 'screen'): Promise<{ release(): Promise<void> }> };
        }
      ).wakeLock;
      return lock?.request('screen').catch(() => undefined);
    },
    visible: () => typeof document !== 'undefined' && document.visibilityState === 'visible',
    onVisible(listener) {
      if (typeof document === 'undefined') return () => {};
      const changed = () => document.visibilityState === 'visible' && listener();
      document.addEventListener('visibilitychange', changed);
      return () => document.removeEventListener('visibilitychange', changed);
    },
    onLeave(listener) {
      if (typeof window === 'undefined') return () => {};
      window.addEventListener('pagehide', listener);
      document.addEventListener('freeze', listener);
      return () => {
        window.removeEventListener('pagehide', listener);
        document.removeEventListener('freeze', listener);
      };
    },
  };
}

/**
 * Page-lifetime camera sharing. It owns the share link, which outlives the
 * camera: when the camera disconnects the share stays up and reports it offline,
 * and the same camera can be shared again on the same link. Component hot swaps
 * do not touch it.
 */
export function createShareSession(tools: DevToolsController, environment = browserEnvironment()) {
  return createRoot((dispose) => {
    const [url, setUrl] = createSignal('');
    const [controllers, setControllers] = createSignal<ReadonlyMap<string, string>>(new Map());
    const [status, setStatus] = createSignal<ShareStatus>();
    const [problem, setProblem] = createSignal<Problem>();
    const [countdown, setCountdown] = createSignal<Countdown>();
    /** What remote tools are doing, by controller: an activity title and a progress bar. */
    const [activities, setActivities] = createSignal<ReadonlyMap<string, string>>(new Map());
    const [progresses, setProgresses] = createSignal<
      ReadonlyMap<string, { label: string; done: number; total: number }>
    >(new Map());
    const ownerCountdown = createOwnerCountdown(setCountdown);
    let countdownController: string | undefined;
    const [registration, setRegistration] = createSignal<Registration>();
    // Signals are committed in batches; operations use these plain values.
    let link = '';
    let current: Registration | undefined;
    let host: ShareHost | undefined;
    let ticket: ReturnType<typeof newTicket> | undefined;
    let identity: { name: string | undefined; firmware: string | undefined } | undefined;
    let camera: Registration['camera'] = { model: null, serial: null };
    let attempt = 0;
    let pending: Promise<Registration> | undefined;
    let lock: { release(): Promise<void> } | undefined;

    const raise = (next: Problem) => {
      setProblem(next);
      if (next.urgent) environment.chime();
    };
    const publish = (next: ShareStatus) => {
      setStatus(next);
      const record: Registration['status'] = { online: next.online };
      if (next.reason) record.reason = next.reason;
      if (next.battery) {
        record.battery = next.battery;
        record.battery_at_ms = Date.now() - (next.battery_age_ms ?? 0);
      }
      if (!link) return;
      current = { url: link, camera, status: record };
      setRegistration(current);
    };
    const currentIdentity = () => ({
      name: tools.camera()?.name,
      firmware: tools.camera()?.firmware?.id,
    });

    /** The link this page shares (created on first use), available before any I/O. */
    function address() {
      ticket ??= newTicket(signalUrl(environment.location()));
      return shareUrl(environment.location(), ticket);
    }

    async function share(): Promise<Registration> {
      if (pending) return pending;
      // One share per page: while it exists (even with the camera away) reuse it.
      if (host && current) return current;
      const mine = ++attempt;
      const link_ = address();
      pending = (async () => {
        const device = await tools.shareUsb();
        if (mine !== attempt) {
          await device.close();
          throw Error('Camera disconnected');
        }
        host = await ShareHost.create(ticket!, device, {
          connected: (id, reason) => {
            setControllers((map) => new Map(map).set(id, reason));
            tools.connectionMessage(
              reason ? `Remote controller connected: ${reason}` : 'Remote controller connected',
            );
          },
          disconnected: (id, reason) => {
            if (countdownController === id) ownerCountdown.cancel();
            setControllers((map) => {
              const next = new Map(map);
              next.delete(id);
              return next;
            });
            tools.shareTrafficEnded(id);
            tools.connectionMessage(`Remote controller disconnected: ${reason}`);
          },
          traffic: (event) => tools.shareTraffic(event),
          toolStatus: (_id, message) => tools.connectionMessage(message, Level.Info),
          activity: (id, title) =>
            setActivities((map) => {
              const next = new Map(map);
              if (title) next.set(id, title);
              else next.delete(id);
              return next;
            }),
          progress: (id, value) =>
            setProgresses((map) => {
              const next = new Map(map);
              if (value) next.set(id, value);
              else next.delete(id);
              return next;
            }),
          countdown: async (id, message) => {
            tools.connectionMessage(message, Level.Info);
            countdownController = id;
            try {
              await ownerCountdown.start(message);
            } finally {
              countdownController = undefined;
            }
          },
          state: publish,
          attention: (_id, message) =>
            raise({ kind: ProblemKind.Attention, message, urgent: true, reconnect: false }),
          error: (error) => tools.connectionMessage(error.message, Level.Error),
        });
        if (mine !== attempt || tools.connection() !== ConnectionState.Sharing) {
          host.stop();
          host = undefined;
          throw Error('Camera disconnected');
        }
        tools.sharingReady(() => host!.claimLocal());
        identity = currentIdentity();
        camera = {
          model:
            tools.camera()?.firmware?.model ?? device.descriptor.product ?? identity.name ?? null,
          serial: device.descriptor.serial,
        };
        link = link_;
        setUrl(link_);
        publish(host.status());
        return current!;
      })();
      pending.catch(() => {
        if (mine === attempt) stop();
      });
      return pending.finally(() => {
        if (mine === attempt) pending = undefined;
      });
    }

    /** Revoke the link and end remote sessions; the local camera stays connected. */
    function stop() {
      ownerCountdown.cancel();
      attempt++;
      pending = undefined;
      host?.stop();
      host = undefined;
      ticket = undefined;
      identity = undefined;
      link = '';
      current = undefined;
      setUrl('');
      setControllers(new Map());
      setStatus(undefined);
      setRegistration(undefined);
      setProblem(undefined);
      tools.sharingEnded();
    }

    /** Called from the alert's button: reopen the camera and share it on the same link. */
    async function reconnect() {
      if (!host) return;
      await tools.connect(CameraTarget.Usb);
      if (tools.connection() !== ConnectionState.Connected) return;
      const now = currentIdentity();
      if (identity && (now.name !== identity.name || now.firmware !== identity.firmware)) {
        raise({
          kind: ProblemKind.DifferentCamera,
          message: `This is a different camera (${now.name ?? 'unknown'}). Connect the camera that was shared.`,
          urgent: true,
          reconnect: true,
        });
        return;
      }
      host.cameraReturned(await tools.shareUsb());
      tools.sharingReady(() => host!.claimLocal());
      publish(host.status());
      setProblem(undefined);
    }

    // The camera left while shared: keep the link, report why, and alert.
    createEffect(
      () => tools.connection(),
      (state) => {
        if (!host || host.status().online === false) return;
        if (state === ConnectionState.Connected || state === ConnectionState.Sharing) return;
        if (state === ConnectionState.Opening || state === ConnectionState.Choosing) return;
        const battery = host.status().battery;
        const exhausted = battery !== undefined && EXHAUSTED.has(battery);
        ownerCountdown.cancel();
        host.cameraLost(exhausted ? 'battery' : 'unplugged');
        publish(host.status());
        setControllers(new Map());
        raise(
          exhausted
            ? {
                kind: ProblemKind.Battery,
                message:
                  'Camera battery ran out. Charge it and turn it on, then reconnect it to keep sharing on the same link.',
                urgent: false,
                reconnect: true,
              }
            : {
                kind: ProblemKind.Unplugged,
                message: 'Camera disconnected. Reconnect it to keep sharing on the same link.',
                urgent: true,
                reconnect: true,
              },
        );
      },
    );

    // Keep the computer awake only while a remote controller is connected.
    const acquire = async () => {
      if (lock || controllers().size === 0 || !environment.visible()) return;
      lock = await environment.wakeLock();
      if (lock && controllers().size === 0) {
        void lock.release();
        lock = undefined;
      }
    };
    createEffect(
      () => controllers().size,
      (count) => {
        if (count > 0) void acquire();
        else if (lock) {
          void lock.release();
          lock = undefined;
        }
      },
    );
    const unlistenVisible = environment.onVisible(() => {
      lock = undefined; // Browsers release the lock while hidden.
      void acquire();
    });
    // Leaving or freezing the page: end remote sessions and the camera session cleanly.
    const unlistenLeave = environment.onLeave(() => {
      if (host) stop();
      tools.disconnect();
    });

    return {
      url,
      controllers,
      status,
      problem,
      countdown,
      /** The latest activity title and progress bar of any remote tool. */
      activity: () => [...activities().values()].at(-1),
      progress: () => [...progresses().values()].at(-1),
      registration,
      sharing: () => url() !== '',
      /** Whether a share exists now, for operation guards (not batched). */
      active: () => link !== '',
      address,
      share,
      stop,
      unlockAudio: () => environment.unlockAudio(),
      reconnect,
      dismissProblem: () => setProblem(undefined),
      dispose() {
        unlistenVisible();
        unlistenLeave();
        if (host) stop();
        dispose();
      },
    };
  });
}
export type ShareSession = ReturnType<typeof createShareSession>;
