export function fileGuide(agent: string): string {
  if (/iPhone|iPad|iPod/.test(agent))
    return 'Use an SD card reader. In Files, open Browse → Locations → your SD card, then FUJIMOD → REPORTS.';
  if (/Android/.test(agent))
    return 'Use an SD card reader. In your file picker, open the SD card, then FUJIMOD → REPORTS.';
  if (/Windows/.test(agent))
    return 'In File Explorer, open This PC → your SD card → FUJIMOD → REPORTS.';
  if (/Macintosh|Mac OS X/.test(agent))
    return 'In Finder, open your SD card under Locations, then FUJIMOD → REPORTS.';
  return 'In your file manager, open the SD card, then FUJIMOD → REPORTS.';
}
