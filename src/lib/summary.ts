import type { DownloadOptions } from "./types";

/** Short human-readable chips describing a set of options. */
export function optionsSummary(o: DownloadOptions): string[] {
  const chips: string[] = [];
  if (o.mode === "video") {
    chips.push(o.quality === "best" ? "Best quality" : `≤ ${o.quality}p`);
    chips.push(o.container.toUpperCase());
    if (o.videoCodec !== "any") chips.push(o.videoCodec === "av1" ? "AV1" : o.videoCodec.toUpperCase());
  } else {
    chips.push("Audio only");
    chips.push(o.audioFormat === "best" ? "Original format" : o.audioFormat.toUpperCase());
  }
  if (o.subtitles !== "none") chips.push(o.subtitles === "embed" ? "Subs embedded" : "Subs as files");
  if (o.sponsorBlock !== "off") chips.push(o.sponsorBlock === "mark" ? "SponsorBlock chapters" : "SponsorBlock cut");
  if (o.cookiesFromBrowser) chips.push(`Cookies: ${o.cookiesFromBrowser}`);
  if (o.extraArgs.trim()) chips.push("Custom args");
  return chips;
}
