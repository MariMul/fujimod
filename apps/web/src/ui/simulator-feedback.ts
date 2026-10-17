/** Keep the attachment instructions when prefilling the simulator template. */
export function simulatorIssueURL(description: string): string {
  const url = new URL("https://github.com/justjake/fujimod/issues/new");
  url.searchParams.set("template", "simulator.md");
  url.searchParams.set("labels", "Simulator");
  url.searchParams.set("body", `## Feedback

${description.trim()}

## Simulator report

Drag and drop the downloaded simulator report ZIP here to upload it before submitting this issue.
`);
  return url.href;
}
