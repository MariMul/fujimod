// @generated
// This file is code generated. Do not edit directly.
// Update the feature documentation sources and regenerate the catalog.
import { For, Show, createSignal, onCleanup, untrack } from 'solid-js';
import type { JSX } from '@solidjs/web';
import featureIcons from '../ui/assets/feature-icons.svg?url';
import { FeatureTile } from './FeatureTile';
import { fadeLoop } from './fade-loop';
import { featureMedia, type Media } from './feature-media';
import './features.css';

type Card = {
  title: string;
  path: string[];
  summary: string | null;
  choices: Record<string, string>;
  related: { card: string; reason: string }[];
  manual: string | null;
  changes: string | null;
};
/** What a hover card shows: a control, or one option of it. */
type Target = { card: string; choice?: string; anchor: HTMLElement };

const MANUAL = 'https://fujifilm-dsc.com/en/manual/x70/';
const MEDIA = '/artifacts/docs/';
/** An X70 manual page, optionally with an anchor ("menu_setup/screen_set-up#framing_guidline"). */
const manualUrl = (manual: string) => {
  const [page, anchor] = manual.split('#');
  return `${MANUAL}${page}/index.html${anchor ? `#${anchor}` : ''}`;
};
const CARDS: Record<string, Card> = {
  image_size: {
    title: 'IMAGE SIZE',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {},
    related: [],
    manual: 'menu_shooting/image_size',
    changes:
      'Aspect ratio selection moved to SHAPE & ASPECT > ASPECT RATIO; IMAGE SIZE now sets L, M or S only.',
  },
  shape_aspect_item: {
    title: 'ASPECT RATIO',
    path: ['SHOOTING MENU', 'SHAPE & ASPECT'],
    summary: 'Choose the shape of still pictures: wide, square or tall.',
    choices: {
      '3:2': "The sensor's full frame, uncropped.",
      '16:9': 'Widescreen, like a TV or movie.',
      '1:1': 'A square frame.',
      '4:3': 'A taller frame, like most phones.',
      '65:24': 'A panoramic strip, as on the Hasselblad XPan.',
      '17:6': 'A wide panoramic strip.',
      '3:4': 'A tall portrait frame.',
      '7:6': 'A near-square frame, like 6×7 film.',
      '5:4': 'A frame like 4×5 large-format film.',
    },
    related: [
      {
        card: 'image_size',
        reason: 'set the max width and height before applying the crop',
      },
    ],
    manual: null,
    changes: null,
  },
  shape_surround_item: {
    title: 'SURROUND VIEW',
    path: ['SHOOTING MENU', 'SHAPE & ASPECT'],
    summary: 'Choose how the area outside the frame is shown.',
    choices: {
      MASKED: 'The area outside the frame is black.',
      DARKENED: 'The area outside the frame is darkened.',
      'WHITE BORDER': 'A white line marks the frame.',
    },
    related: [],
    manual: null,
    changes: null,
  },
  shape_desqueeze_item: {
    title: 'DESQUEEZE',
    path: ['SHOOTING MENU', 'SHAPE & ASPECT'],
    summary: 'Choose the squeeze factor of your anamorphic adapter.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  shape_jpeg_item: {
    title: 'DESQUEEZE JPEG',
    path: ['SHOOTING MENU', 'SHAPE & ASPECT'],
    summary: 'Choose whether JPEG pictures are also recorded at their corrected width.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  c: {
    title: 'FRAMING GUIDELINE',
    path: ['SET-UP', 'SCREEN SET-UP'],
    summary: null,
    choices: {
      'GOLDEN RATIO': 'Lines that divide the frame at the golden ratio.',
      'GOLDEN SPIRAL - BOTTOM LEFT':
        'A golden logarithmic spiral with its focal point at the lower-left golden-section intersection.',
      'GOLDEN SPIRAL - BOTTOM RIGHT':
        'A golden logarithmic spiral with its focal point at the lower-right golden-section intersection.',
      'GOLDEN SPIRAL - TOP LEFT':
        'A golden logarithmic spiral with its focal point at the upper-left golden-section intersection.',
      'GOLDEN SPIRAL - TOP RIGHT':
        'A golden logarithmic spiral with its focal point at the upper-right golden-section intersection.',
    },
    related: [],
    manual: 'menu_setup/screen_set-up#framing_guidline',
    changes:
      'Adds GOLDEN RATIO and four GOLDEN SPIRAL focal-point placements. All guides follow the selected image bounds and aspect ratio.',
  },
  film_simulation: {
    title: 'FILM SIMULATION',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {},
    related: [],
    manual: 'menu_shooting/film_simulation',
    changes:
      'Adds ETERNA, Neutral and approximations of four film simulations from newer cameras, and films saved on the memory card. Approximations are marked (approx.) in the camera.',
  },
  film_editor_item: {
    title: 'FILM EDITOR',
    path: ['SHOOTING MENU'],
    summary: 'Create and edit film simulations, saved as files on the memory card.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  photometry: {
    title: 'PHOTOMETRY',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {
      HIGHLIGHT:
        'Exposure is reduced when bright areas dominate the scene. A small bright detail may still be overexposed.',
      'RAW HIGHLIGHT':
        'Expose To The Right (ETTR): Exposure is set so that the brightest areas of the frame are recorded as brightly as possible without loss of detail in RAW images.',
    },
    related: [
      {
        card: 'raw_view',
        reason: 'see how close the brightest areas are to losing detail in RAW file while shooting',
      },
    ],
    manual: 'menu_shooting/photometry',
    changes:
      'Adds HIGHLIGHT, which reduces exposure when bright areas dominate the scene. The shooting, Fn and Q displays mark it with an asterisk in brackets.',
  },
  raw_view: {
    title: 'RAW VIEW',
    path: [],
    summary:
      'Press a button assigned to RAW VIEW to see how close the brightest areas are to losing detail in the RAW file. Press it again to return to the normal live view.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  f_log_recording_item: {
    title: 'F-Log RECORDING',
    path: ['SHOOTING MENU', 'MOVIE SET-UP'],
    summary: 'Choose F-Log processing for movies.',
    choices: {
      OFF: 'Use the selected picture settings.',
      ON: 'Use F-Log movie processing.',
    },
    related: [],
    manual: null,
    changes: null,
  },
  hdmi_rec_behavior_item: {
    title: 'HDMI OUTPUT',
    path: ['SET-UP'],
    summary: 'Choose whether an HDMI device shows the shooting view or playback only.',
    choices: {
      'LIVE VIEW': 'The HDMI device shows the shooting view.',
      'PLAYBACK ONLY': 'The HDMI device shows playback only, as on an unmodified camera.',
    },
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_exposure_compensation_dial: {
    title: 'EV DIAL',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose an action and the value each EV notch selects.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_command_control: {
    title: 'COMMAND CONTROL',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose the setting the command control adjusts.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_auto_mode_selector_lever: {
    title: 'AUTO LEVER',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose an action and the value each Auto-lever position selects.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_shutter_speed_dial: {
    title: 'SHUTTER DIAL',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose an action and the value each shutter notch selects.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_aperture_ring: {
    title: 'APERTURE RING',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose an action and the value each aperture notch selects.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_focus_mode_selector: {
    title: 'FOCUS MODE',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose an action and the value S, C and M each select.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_af_ae_lock_button: {
    title: 'AE-L/AF-L',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose the AE-L/AF-L button action and press operation.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_drive_button: {
    title: 'DRIVE BUTTON',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose the DRIVE button action and press operation.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_quick_menu_button: {
    title: 'Q BUTTON',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose the Q button action and press operation.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_command_control_press: {
    title: 'COMMAND PRESS',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose the command control press action and press operation.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  control_ring_setting: {
    title: 'CONTROL RING SETTING',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {},
    related: [],
    manual: 'menu_shooting/control_ring_setting',
    changes:
      'Adds EXPOSURE COMP., STILLS/MOVIE and the FujiMod settings ASPECT RATIO, SELECT RECIPE and SURROUND VIEW.',
  },
  edit_save_custom_setting: {
    title: 'EDIT/SAVE CUSTOM SETTING',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {},
    related: [],
    manual: 'menu_shooting/edit-save_custom_setting',
    changes:
      'Each slot C1–C7 holds a recipe saved as a named file on the memory card. Choose SAVE CURRENT SETTINGS, SAVE AS NEW RECIPE, CHOOSE FROM SD CARD or CLEAR SLOT.',
  },
  snap_menu_item: {
    title: 'MF SNAP FOCUS',
    path: ['SHOOTING MENU'],
    summary: 'Choose the distance used by the SNAP FOCUS action in manual focus mode.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bug_report_shortcut: {
    title: 'BUG REPORT SHORTCUT',
    path: ['SET-UP', 'FUJIMOD DEVTOOLS'],
    summary: 'Choose whether the side Fn button and DISP/BACK save a bug report.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  sd_volume_name_item: {
    title: 'DATE CARD NAME',
    path: ['SET-UP', 'SAVE DATA SET-UP'],
    summary: 'Choose whether FORMAT names the memory card after the date, such as 2026-10-22.',
    choices: {
      ON: 'Formatted cards are named after the date.',
      OFF: 'Formatted cards are left unnamed, as on an unmodified camera.',
    },
    related: [],
    manual: null,
    changes: null,
  },
  date_folders_item: {
    title: 'DATE FOLDERS',
    path: ['SET-UP', 'SAVE DATA SET-UP'],
    summary:
      'Choose whether the first picture of each day starts a new folder named after the date.',
    choices: {
      ON: 'Each day starts a new folder, such as 101_1022 for October 22.',
      OFF: 'Pictures are stored in folders as on an unmodified camera.',
    },
    related: [],
    manual: null,
    changes: null,
  },
};
/** Recordings rendered for exactly this catalog text, else none. */
const media = featureMedia('63eaf5bdb6f719c7103954d9');

function Icon(props: { name: string }) {
  return (
    <svg class="catalog-icon" aria-hidden="true">
      <use href={`${featureIcons}#${props.name}`} />
    </svg>
  );
}

/** An aspect ratio choice drawn as its frame. */
function Ratio(props: { width: number; height: number }) {
  const scale = () => 14 / Math.max(props.width, props.height);
  return (
    <svg class="catalog-ratio" viewBox="0 0 14 14" aria-hidden="true">
      <rect
        x={(14 - props.width * scale()) / 2}
        y={(14 - props.height * scale()) / 2}
        width={props.width * scale()}
        height={props.height * scale()}
        rx="1"
      />
    </svg>
  );
}

function CardVideo(props: { media: Media; choice?: string }) {
  let video: HTMLVideoElement | undefined;
  // A site without feature media (or a missing file) shows no video at all.
  const [failed, setFailed] = createSignal(false);
  // Read once: the card's own range, still valid while its video is closing.
  const range = untrack(() => (props.choice ? props.media.clips[props.choice] : undefined));
  const clip = () => range;
  let loop: ReturnType<typeof fadeLoop> | undefined;
  onCleanup(() => loop?.dispose());
  return (
    <Show when={!failed()}>
      <video
        ref={(element) => {
          video = element;
          loop = fadeLoop(element, () => clip()?.[0] ?? 0);
        }}
        class="catalog-card-video"
        src={MEDIA + props.media.video}
        poster={MEDIA + props.media.still}
        muted
        autoplay
        playsinline
        preload="metadata"
        onLoadedMetadata={() => {
          const range = clip();
          if (video && range) video.currentTime = range[0];
        }}
        onTimeUpdate={() => {
          const range = clip();
          if (video && range && video.currentTime >= range[1]) loop?.restart();
        }}
        onError={() => setFailed(true)}
      />
    </Show>
  );
}

function CardBody(props: { target: Target }) {
  const card = () => CARDS[props.target.card];
  const choice = () => props.target.choice;
  const path = () => [...card().path, card().title, ...(choice() ? [choice()!] : [])].join(' › ');
  return (
    <>
      <Show when={media(props.target.card)}>
        {(found) => <CardVideo media={found()} choice={choice()} />}
      </Show>
      <div class="catalog-card-title">{choice() ?? card().title}</div>
      <Show
        when={choice()}
        fallback={
          <Show when={card().changes} fallback={<p>{card().summary}</p>}>
            <p>In FujiMod: {card().changes}</p>
            <a href={manualUrl(card().manual!)} target="_blank" rel="noreferrer">
              X70 manual
            </a>
          </Show>
        }
      >
        <p>{card().choices[choice()!]}</p>
      </Show>
      <div class="catalog-card-path">{path()}</div>
      <Show when={!choice() && card().related.length}>
        <ul class="catalog-card-related">
          <For each={card().related}>
            {(related) => (
              <li>
                <b>{CARDS[related.card].title}</b>: {related.reason}
              </li>
            )}
          </For>
        </ul>
      </Show>
    </>
  );
}

export function FeatureCatalog() {
  const [target, setTarget] = createSignal<Target>();
  const [position, setPosition] = createSignal({ left: 0, top: 0 });
  let timer: ReturnType<typeof setTimeout> | undefined;
  let card: HTMLDivElement | undefined;
  onCleanup(() => clearTimeout(timer));

  const place = (anchor: HTMLElement) => {
    const r = anchor.getBoundingClientRect();
    const width = Math.min(340, innerWidth - 16);
    const height = card?.offsetHeight ?? 0;
    let top = r.bottom + 10;
    if (top + height > innerHeight - 8) top = Math.max(8, r.top - height - 10);
    setPosition({
      left: Math.min(Math.max(8, r.left + r.width / 2 - width / 2), innerWidth - width - 8),
      top,
    });
  };
  const show = (next: Target) => {
    setTarget(next);
    place(next.anchor);
    requestAnimationFrame(() => place(next.anchor));
  };
  const later = (action: () => void, ms: number) => {
    clearTimeout(timer);
    timer = setTimeout(action, ms);
  };
  /** A control or option with a hover card: after a short hover, on focus or on tap. */
  const Ref = (props: { card: string; choice?: string; class?: string; children: JSX.Element }) => (
    <span
      class={props.class ? `${props.class} catalog-ref` : 'catalog-ref'}
      tabindex="0"
      onMouseEnter={(event) => {
        const anchor = event.currentTarget;
        later(() => show({ card: props.card, choice: props.choice, anchor }), 250);
      }}
      onMouseLeave={() => later(() => setTarget(), 150)}
      onFocus={(event) =>
        show({ card: props.card, choice: props.choice, anchor: event.currentTarget })
      }
      onBlur={() => later(() => setTarget(), 150)}
      onClick={(event) => {
        const anchor = event.currentTarget;
        if (target()?.anchor === anchor) setTarget();
        else show({ card: props.card, choice: props.choice, anchor });
      }}
    >
      {props.children}
    </span>
  );

  return (
    <section class="catalog" id="features" aria-labelledby="catalog-heading">
      <h2 id="catalog-heading">Features</h2>
      <h3>Making pictures</h3>
      <div class="catalog-grid">
        <FeatureTile media={media('film_simulation')}>
          <h4>
            <Icon name="film" />
            <Ref card="film_simulation">FILM SIMULATION</Ref>
          </h4>
          <p>
            Simulate the effects of additional types of film. Create your own film simulations in
            FILM EDITOR.
          </p>
          <div class="catalog-chips">
            <span class="catalog-chip">
              <Icon name="eterna" />
              ETERNA
            </span>
            <span class="catalog-chip">
              <Icon name="classic-neg" />
              Classic Neg. (approx.)
            </span>
            <span class="catalog-chip">
              <Icon name="bleach-bypass" />
              Bleach Bypass (approx.)
            </span>
            <span class="catalog-chip">
              <Icon name="nostalgic-neg" />
              Nostalgic Neg. (approx.)
            </span>
            <span class="catalog-chip">
              <Icon name="reala-ace" />
              REALA ACE (approx.)
            </span>
            <span class="catalog-chip">Neutral</span>
          </div>
          <div class="catalog-where">SHOOTING MENU</div>
        </FeatureTile>
        <FeatureTile media={media()}>
          <h4>
            <Icon name="film" />
            <span>Card Films</span>
          </h4>
          <p>Load custom film simulations from the memory card.</p>
        </FeatureTile>
        <FeatureTile media={media('film_editor_item')}>
          <h4>
            <Icon name="film" />
            <Ref card="film_editor_item">FILM EDITOR</Ref>
          </h4>
          <p>Create and edit your own film simulations, saved as files on the memory card.</p>
          <div class="catalog-where">SHOOTING MENU</div>
        </FeatureTile>
        <FeatureTile media={media('photometry')}>
          <h4>
            <Icon name="highlight" />
            <Ref card="photometry">PHOTOMETRY</Ref>
          </h4>
          <p>Choose HIGHLIGHT to reduce exposure when bright areas dominate the scene.</p>
          <div class="catalog-chips">
            <span class="catalog-chip stock">MULTI</span>
            <span class="catalog-chip stock">SPOT</span>
            <span class="catalog-chip stock">AVERAGE</span>
            <Ref card="photometry" choice="HIGHLIGHT" class="catalog-chip">
              HIGHLIGHT
            </Ref>
            <Ref card="photometry" choice="RAW HIGHLIGHT" class="catalog-chip">
              RAW HIGHLIGHT
            </Ref>
          </div>
          <div class="catalog-where">SHOOTING MENU</div>
        </FeatureTile>
        <FeatureTile media={media('raw_view')}>
          <h4>
            <Icon name="highlight" />
            <Ref card="raw_view">RAW VIEW</Ref>
          </h4>
          <p>
            Assign RAW VIEW to a function button to see how close the brightest areas are to losing
            detail in the RAW file.
          </p>
        </FeatureTile>
        <FeatureTile media={media('edit_save_custom_setting')}>
          <h4>
            <Icon name="recipes" />
            <span>Unlimited Recipes</span>
          </h4>
          <p>
            Store custom settings as named files on the memory card and load them into{' '}
            <span class="nowrap">C1–C7</span>.
          </p>
          <div class="catalog-chips">
            <span class="catalog-chip action">SAVE CURRENT SETTINGS</span>
            <span class="catalog-chip action">SAVE AS NEW RECIPE</span>
            <span class="catalog-chip action">CHOOSE FROM SD CARD</span>
          </div>
          <div class="catalog-where">SHOOTING MENU › EDIT/SAVE CUSTOM SETTING</div>
        </FeatureTile>
        <FeatureTile media={media('shape_aspect_item')}>
          <h4>
            <Icon name="aspect" />
            <Ref card="shape_aspect_item">ASPECT RATIO</Ref>
          </h4>
          <p>Choose the aspect ratio of still pictures.</p>
          <div class="catalog-chips">
            <Ref card="shape_aspect_item" choice="3:2" class="catalog-chip stock">
              <Ratio width={3} height={2} />
              3:2
            </Ref>
            <Ref card="shape_aspect_item" choice="16:9" class="catalog-chip stock">
              <Ratio width={16} height={9} />
              16:9
            </Ref>
            <Ref card="shape_aspect_item" choice="1:1" class="catalog-chip stock">
              <Ratio width={1} height={1} />
              1:1
            </Ref>
            <Ref card="shape_aspect_item" choice="4:3" class="catalog-chip">
              <Ratio width={4} height={3} />
              4:3
            </Ref>
            <Ref card="shape_aspect_item" choice="65:24" class="catalog-chip">
              <Ratio width={65} height={24} />
              65:24
            </Ref>
            <Ref card="shape_aspect_item" choice="17:6" class="catalog-chip">
              <Ratio width={17} height={6} />
              17:6
            </Ref>
            <Ref card="shape_aspect_item" choice="3:4" class="catalog-chip">
              <Ratio width={3} height={4} />
              3:4
            </Ref>
            <Ref card="shape_aspect_item" choice="7:6" class="catalog-chip">
              <Ratio width={7} height={6} />
              7:6
            </Ref>
            <Ref card="shape_aspect_item" choice="5:4" class="catalog-chip">
              <Ratio width={5} height={4} />
              5:4
            </Ref>
          </div>
          <div class="catalog-where">{'SHOOTING MENU › SHAPE & ASPECT'}</div>
        </FeatureTile>
        <FeatureTile media={media('shape_surround_item')}>
          <h4>
            <Icon name="aspect" />
            <Ref card="shape_surround_item">SURROUND VIEW</Ref>
          </h4>
          <p>Choose how the area outside the frame is shown.</p>
          <div class="catalog-chips">
            <Ref card="shape_surround_item" choice="MASKED" class="catalog-chip">
              MASKED
            </Ref>
            <Ref card="shape_surround_item" choice="DARKENED" class="catalog-chip">
              DARKENED
            </Ref>
            <Ref card="shape_surround_item" choice="WHITE BORDER" class="catalog-chip">
              WHITE BORDER
            </Ref>
          </div>
          <div class="catalog-where">{'SHOOTING MENU › SHAPE & ASPECT'}</div>
        </FeatureTile>
        <FeatureTile media={media('c')}>
          <h4>
            <Icon name="golden-ratio" />
            <Ref card="c">FRAMING GUIDELINE</Ref>
          </h4>
          <p>
            Choose a guide to display in the shooting view, including Golden Ratio and four Golden
            Spiral focal points.
          </p>
          <div class="catalog-chips">
            <span class="catalog-chip stock">GRID 9</span>
            <span class="catalog-chip stock">GRID 24</span>
            <span class="catalog-chip stock">HD FRAMING</span>
            <Ref card="c" choice="GOLDEN RATIO" class="catalog-chip">
              GOLDEN RATIO
            </Ref>
            <Ref card="c" choice="GOLDEN SPIRAL - BOTTOM LEFT" class="catalog-chip">
              GOLDEN SPIRAL - BOTTOM LEFT
            </Ref>
            <Ref card="c" choice="GOLDEN SPIRAL - BOTTOM RIGHT" class="catalog-chip">
              GOLDEN SPIRAL - BOTTOM RIGHT
            </Ref>
            <Ref card="c" choice="GOLDEN SPIRAL - TOP LEFT" class="catalog-chip">
              GOLDEN SPIRAL - TOP LEFT
            </Ref>
            <Ref card="c" choice="GOLDEN SPIRAL - TOP RIGHT" class="catalog-chip">
              GOLDEN SPIRAL - TOP RIGHT
            </Ref>
          </div>
          <div class="catalog-where">SET-UP › SCREEN SET-UP</div>
        </FeatureTile>
        <FeatureTile media={media('shape_desqueeze_item')}>
          <h4>
            <Icon name="anamorphic" />
            <span>Anamorphic Lenses</span>
          </h4>
          <p>
            Display and record pictures taken through an anamorphic adapter at their corrected
            width.
          </p>
          <div class="catalog-sub">
            <h5>
              <Ref card="shape_desqueeze_item">DESQUEEZE</Ref>
            </h5>
            <div class="catalog-chips">
              <span class="catalog-chip">1.25X</span>
              <span class="catalog-chip">1.33X</span>
              <span class="catalog-chip">1.50X</span>
              <span class="catalog-chip">1.60X</span>
              <span class="catalog-chip">1.80X</span>
              <span class="catalog-chip">2.00X</span>
            </div>
          </div>
          <div class="catalog-where">{'SHOOTING MENU › SHAPE & ASPECT'}</div>
        </FeatureTile>
        <FeatureTile media={media('shape_jpeg_item')}>
          <h4>
            <Icon name="anamorphic" />
            <span>Desqueeze JPEG</span>
          </h4>
          <p>
            Display and record pictures taken through an anamorphic adapter at their corrected
            width.
          </p>
          <div class="catalog-sub">
            <h5>
              <Ref card="shape_jpeg_item">DESQUEEZE JPEG</Ref>
            </h5>
            <div class="catalog-chips">
              <span class="catalog-chip">ON</span>
              <span class="catalog-chip">OFF</span>
            </div>
          </div>
          <div class="catalog-where">{'SHOOTING MENU › SHAPE & ASPECT'}</div>
        </FeatureTile>
        <FeatureTile media={media('snap_menu_item')}>
          <h4>
            <Icon name="snap" />
            <Ref card="snap_menu_item">MF SNAP FOCUS</Ref>
          </h4>
          <p>
            In manual focus mode, press a button assigned to SNAP FOCUS to focus at a preset
            distance.
          </p>
          <div class="catalog-where">SHOOTING MENU</div>
        </FeatureTile>
        <FeatureTile media={media('date_folders_item')}>
          <h4>
            <Icon name="folder" />
            <Ref card="date_folders_item">DATE FOLDERS</Ref>
          </h4>
          <p>{"Keep each day's pictures in their own folder, named after the date."}</p>
          <div class="catalog-where">SET-UP › SAVE DATA SET-UP</div>
        </FeatureTile>
      </div>
      <h3>Making movies</h3>
      <div class="catalog-grid">
        <FeatureTile media={media('f_log_recording_item')}>
          <h4>
            <Icon name="movie" />
            <Ref card="f_log_recording_item">F-Log RECORDING</Ref>
          </h4>
          <p>Record movies with the F-Log transfer and F-Gamut color matrix.</p>
          <div class="catalog-where">SHOOTING MENU › MOVIE SET-UP</div>
        </FeatureTile>
      </div>
      <h3>Handling and display</h3>
      <div class="catalog-grid">
        <FeatureTile
          media={media(
            'bindings_row_exposure_compensation_dial',
            'bindings_row_command_control',
            'control_ring_setting',
            'bindings_row_auto_mode_selector_lever',
            'bindings_row_shutter_speed_dial',
            'bindings_row_aperture_ring',
            'bindings_row_focus_mode_selector',
            'bindings_row_af_ae_lock_button',
            'bindings_row_drive_button',
            'bindings_row_quick_menu_button',
            'bindings_row_command_control_press',
          )}
        >
          <h4>
            <Icon name="controls" />
            <span>Unlocked Controls</span>
          </h4>
          <p>
            Assign settings to the EV dial, shutter dial, aperture ring, command control, control
            ring, focus selector, Auto lever and unlocked buttons.
          </p>
        </FeatureTile>
        <FeatureTile media={media('hdmi_rec_behavior_item')}>
          <h4>
            <Icon name="hdmi" />
            <Ref card="hdmi_rec_behavior_item">HDMI OUTPUT</Ref>
          </h4>
          <p>Display the shooting view on an HDMI device.</p>
          <div class="catalog-where">SET-UP</div>
        </FeatureTile>
        <FeatureTile media={media()}>
          <h4>
            <Icon name="autofocus" />
            <span>Autofocus</span>
          </h4>
          <p>
            In AF-S mode, when phase detection fails, the camera estimates focus and measures again
            before using contrast detection.
          </p>
        </FeatureTile>
        <FeatureTile media={media('sd_volume_name_item')}>
          <h4>
            <Icon name="card" />
            <Ref card="sd_volume_name_item">DATE CARD NAME</Ref>
          </h4>
          <p>Name memory cards after the date they are formatted.</p>
          <div class="catalog-where">SET-UP › SAVE DATA SET-UP</div>
        </FeatureTile>
      </div>
      <h3>When something goes wrong</h3>
      <div class="catalog-grid">
        <FeatureTile media={media('bug_report_shortcut')}>
          <h4>
            <Icon name="bug" />
            <span>Bug Reports</span>
          </h4>
          <p>
            Hold the side Fn button and press DISP/BACK to save a diagnostic report to the memory
            card and display a QR code.
          </p>
          <div class="catalog-where">SET-UP › FUJIMOD DEVTOOLS › BUG REPORT SHORTCUT</div>
        </FeatureTile>
      </div>
      <div
        ref={(element) => {
          card = element;
        }}
        class={['catalog-card', { on: !!target() }]}
        role="tooltip"
        style={{ left: `${position().left}px`, top: `${position().top}px` }}
        onMouseEnter={() => clearTimeout(timer)}
        onMouseLeave={() => later(() => setTarget(), 150)}
      >
        <Show when={target()}>{(current) => <CardBody target={current()} />}</Show>
      </div>
    </section>
  );
}
