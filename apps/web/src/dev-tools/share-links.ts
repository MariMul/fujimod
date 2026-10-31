import type { CameraIdentity } from './camera-model';

/**
 * A persistent share link this browser owns. The owner key lets the page join
 * the room again after a reload; it never leaves this browser.
 */
export interface OwnedShare {
  link: string;
  owner_key: string;
}
export interface ShareLinks {
  find(camera: string): OwnedShare | undefined;
  save(camera: string, share: OwnedShare): void;
  forget(camera: string): void;
}

/** One link per camera body, named by its FF80 product and serial. */
export function shareCamera(identity: CameraIdentity | undefined): string | undefined {
  return identity?.product && identity.serial ? `${identity.product} ${identity.serial}` : undefined;
}

const KEY = 'fujimod.dev-tools.share-links';

export function browserShareLinks(): ShareLinks {
  const read = (): Record<string, OwnedShare> => {
    try {
      const value: unknown = JSON.parse(localStorage.getItem(KEY) ?? '{}');
      return value && typeof value === 'object' ? (value as Record<string, OwnedShare>) : {};
    } catch {
      return {};
    }
  };
  const write = (links: Record<string, OwnedShare>) => {
    if (typeof localStorage !== 'undefined') localStorage.setItem(KEY, JSON.stringify(links));
  };
  return {
    find(camera) {
      const share = read()[camera];
      return typeof share?.link === 'string' && typeof share.owner_key === 'string'
        ? share
        : undefined;
    },
    save(camera, share) {
      write({ ...read(), [camera]: { link: share.link, owner_key: share.owner_key } });
    },
    forget(camera) {
      const links = read();
      delete links[camera];
      write(links);
    },
  };
}

export function memoryShareLinks(): ShareLinks & { links: Map<string, OwnedShare> } {
  const links = new Map<string, OwnedShare>();
  return {
    links,
    find: (camera) => links.get(camera),
    save: (camera, share) => links.set(camera, share),
    forget: (camera) => links.delete(camera),
  };
}
