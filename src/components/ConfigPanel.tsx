import { Film, Music, SlidersHorizontal } from "lucide-react";
import type { AudioFormat, Container, Quality, SponsorBlock, Subtitles, VideoCodec } from "../lib/types";
import { useAppStore, useOptions } from "../store/useAppStore";
import { Field, Panel, Segmented, Select, Toggle } from "./ui";

const QUALITIES: { value: Quality; label: string }[] = [
  { value: "best", label: "Best available" },
  { value: "2160", label: "2160p (4K)" },
  { value: "1440", label: "1440p" },
  { value: "1080", label: "1080p" },
  { value: "720", label: "720p" },
  { value: "480", label: "480p" },
  { value: "360", label: "360p" },
];
const CONTAINERS: { value: Container; label: string }[] = [
  { value: "mp4", label: "MP4 (most compatible)" },
  { value: "mkv", label: "MKV (holds anything)" },
  { value: "webm", label: "WebM" },
];
const CODECS: { value: VideoCodec; label: string }[] = [
  { value: "any", label: "Any (best quality)" },
  { value: "h264", label: "H.264 / AVC" },
  { value: "vp9", label: "VP9" },
  { value: "av1", label: "AV1" },
];
const AUDIO_FORMATS: { value: AudioFormat; label: string }[] = [
  { value: "best", label: "Original (no re-encode)" },
  { value: "mp3", label: "MP3" },
  { value: "m4a", label: "M4A / AAC" },
  { value: "opus", label: "Opus" },
  { value: "flac", label: "FLAC (lossless container)" },
  { value: "wav", label: "WAV" },
];
const SUBTITLES: { value: Subtitles; label: string }[] = [
  { value: "none", label: "No subtitles" },
  { value: "embed", label: "Embed in video" },
  { value: "write", label: "Save as separate files" },
];
const SPONSOR: { value: SponsorBlock; label: string }[] = [
  { value: "off", label: "Off" },
  { value: "mark", label: "Mark segments as chapters" },
  { value: "remove", label: "Cut sponsor segments" },
];

export function ConfigPanel() {
  const o = useOptions();
  const set = useAppStore((s) => s.setOptions);
  const video = o.mode === "video";
  const lossy = ["mp3", "m4a", "opus"].includes(o.audioFormat);

  return (
    <Panel title="Format & quality" icon={<SlidersHorizontal size={17} />}>
      <Segmented
        label="Download mode"
        value={o.mode}
        onChange={(mode) => set({ mode })}
        options={[
          { value: "video", label: "Video", icon: <Film size={15} /> },
          { value: "audio", label: "Audio only", icon: <Music size={15} /> },
        ]}
      />

      <div className="config-grid">
        {video ? (
          <>
            <Field label="Maximum quality">
              <Select value={o.quality} onChange={(quality) => set({ quality })} options={QUALITIES} />
            </Field>
            <Field label="Container">
              <Select value={o.container} onChange={(container) => set({ container })} options={CONTAINERS} />
            </Field>
            <Field label="Preferred video codec" hint="A preference: yt-dlp falls back when it isn't available.">
              <Select value={o.videoCodec} onChange={(videoCodec) => set({ videoCodec })} options={CODECS} />
            </Field>
          </>
        ) : (
          <>
            <Field label="Audio format">
              <Select value={o.audioFormat} onChange={(audioFormat) => set({ audioFormat })} options={AUDIO_FORMATS} />
            </Field>
            <Field label={`Audio quality: ${o.audioQuality === 0 ? "best" : o.audioQuality}`} hint={lossy ? "0 = best, 10 = smallest" : "Only applies to MP3, M4A and Opus"}>
              <input
                type="range"
                min={0}
                max={10}
                value={o.audioQuality}
                disabled={!lossy}
                onChange={(e) => set({ audioQuality: Number(e.target.value) })}
              />
            </Field>
          </>
        )}
        <Field label="Subtitles">
          <Select value={o.subtitles} onChange={(subtitles) => set({ subtitles })} options={SUBTITLES} />
        </Field>
        {o.subtitles !== "none" && (
          <Field label="Subtitle languages" hint='Comma separated, regex allowed. Example: "en.*,nl"'>
            <input value={o.subLangs} onChange={(e) => set({ subLangs: e.target.value })} spellCheck={false} />
          </Field>
        )}
        <Field label="SponsorBlock">
          <Select value={o.sponsorBlock} onChange={(sponsorBlock) => set({ sponsorBlock })} options={SPONSOR} />
        </Field>
      </div>

      <div className="toggle-grid">
        <Toggle label="Embed thumbnail" checked={o.embedThumbnail} onChange={(embedThumbnail) => set({ embedThumbnail })} />
        <Toggle label="Embed metadata" checked={o.embedMetadata} onChange={(embedMetadata) => set({ embedMetadata })} />
        <Toggle label="Embed chapters" checked={o.embedChapters} onChange={(embedChapters) => set({ embedChapters })} />
        <Toggle
          label="Auto-generated subtitles"
          checked={o.autoSubs}
          disabled={o.subtitles === "none"}
          onChange={(autoSubs) => set({ autoSubs })}
        />
        <Toggle
          label="Single video only"
          hint="Ignore the playlist when a URL contains both a video and a list"
          checked={o.noPlaylist}
          onChange={(noPlaylist) => set({ noPlaylist })}
        />
      </div>
    </Panel>
  );
}
