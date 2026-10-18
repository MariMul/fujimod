export interface ReportPlatform {
  name: string;
  fileManager: string;
  mobile: boolean;
  guide: string;
}
export function reportPlatform(agent: string, touchPoints = 0): ReportPlatform {
  if (/iPad/.test(agent) || (/Macintosh/.test(agent) && touchPoints > 1))
    return {
      name: 'iPad',
      fileManager: 'Files',
      mobile: true,
      guide:
        'Use an SD card reader. In Files, open Browse → Locations → your SD card, then FUJIMOD → REPORTS.',
    };
  if (/iPhone|iPod/.test(agent))
    return {
      name: 'iPhone',
      fileManager: 'Files',
      mobile: true,
      guide:
        'Use an SD card reader. In Files, open Browse → Locations → your SD card, then FUJIMOD → REPORTS.',
    };
  if (/Android/.test(agent))
    return {
      name: 'Android',
      fileManager: 'Files',
      mobile: true,
      guide:
        'Use an SD card reader. In your file picker, open the SD card, then FUJIMOD → REPORTS.',
    };
  if (/Windows/.test(agent))
    return {
      name: 'Windows',
      fileManager: 'File Explorer',
      mobile: false,
      guide: 'In File Explorer, open This PC → your SD card → FUJIMOD → REPORTS.',
    };
  if (/Macintosh|Mac OS X/.test(agent))
    return {
      name: 'macOS',
      fileManager: 'Finder',
      mobile: false,
      guide: 'In Finder, open your SD card under Locations, then FUJIMOD → REPORTS.',
    };
  return {
    name: /Linux/.test(agent) ? 'Linux' : 'This device',
    fileManager: 'File manager',
    mobile: false,
    guide: 'In your file manager, open the SD card, then FUJIMOD → REPORTS.',
  };
}
