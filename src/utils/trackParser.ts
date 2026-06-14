import type { TrackMetadata } from "../types";

export function parseTracklistLocally(input: string): TrackMetadata[] {
  const text = input.replace(/\r\n?/g, "\n").replace(/\u3000/g, " ");
  const lines = text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line && !isHeading(line));
  const lineTracks = lines
    .map(parseNumberedLine)
    .filter((track): track is TrackMetadata => Boolean(track));

  if (lineTracks.length > 1) {
    return lineTracks;
  }

  const blockTracks = parseBlockTracks(lines);
  if (blockTracks.length > 1) {
    return blockTracks;
  }

  const stackedTracks = parseStackedTracks(lines);
  if (stackedTracks.length > 1) {
    return stackedTracks;
  }

  const unnumberedBlockTracks = parseUnnumberedMetadataBlocks(lines);
  if (unnumberedBlockTracks.length > 1) {
    return unnumberedBlockTracks;
  }

  return parseContiguous(text);
}

function parseNumberedLine(line: string): TrackMetadata | null {
  const match = line.match(/^(\d{1,3})[\.\)\]:\s\t]*(.+)$/);
  if (!match) return null;

  const parsed = parseRest(match[2]);
  if (!parsed) return null;

  return {
    number: Number(match[1]),
    title: parsed.title,
    artist: parsed.artist,
  };
}

function parseBlockTracks(lines: string[]): TrackMetadata[] {
  const tracks: TrackMetadata[] = [];
  let index = 0;

  while (index < lines.length) {
    const marker = parseNumberMarker(lines[index]);
    if (!marker) {
      index += 1;
      continue;
    }

    const nextOffset = lines.slice(index + 1).findIndex((line) => Boolean(parseNumberMarker(line)));
    const nextIndex = nextOffset >= 0 ? index + 1 + nextOffset : lines.length;
    const block = lines.slice(index + 1, nextIndex);
    const hasMetadata = block.some(isMetadataLine);
    const title =
      marker.title ||
      block.find((line) => !isMetadataLine(line) && !parseNumberMarker(line))?.trim();

    if (hasMetadata && title) {
      tracks.push({
        number: marker.number,
        title: clean(title),
        artist: extractArtistFromBlock(block) || "",
      });
    }

    index = nextIndex;
  }

  return tracks;
}

function parseStackedTracks(lines: string[]): TrackMetadata[] {
  const tracks: TrackMetadata[] = [];
  const seed = inferArtistSeed(lines.filter((line) => !parseNumberMarker(line)));
  let index = 0;

  while (index < lines.length) {
    const marker = parseNumberMarker(lines[index]);
    if (!marker) {
      index += 1;
      continue;
    }

    if (marker.title) {
      const parsed = parseRest(marker.title, seed);
      if (parsed) {
        tracks.push({
          number: marker.number,
          title: parsed.title,
          artist: parsed.artist,
        });
        index += 1;
        continue;
      }
    }

    if (!marker.title && index + 1 < lines.length && !parseNumberMarker(lines[index + 1])) {
      const parsed = parseRest(lines[index + 1], seed);
      if (parsed) {
        tracks.push({
          number: marker.number,
          title: parsed.title,
          artist: parsed.artist,
        });
        index += 2;
        continue;
      }
    }

    const titleIndex = marker.title ? index : index + 1;
    const artistIndex = titleIndex + 1;
    if (artistIndex >= lines.length) break;

    const title = marker.title || lines[titleIndex];
    const artist = lines[artistIndex];
    if (
      !title.trim() ||
      !artist.trim() ||
      parseNumberMarker(artist) ||
      (!marker.title && parseNumberMarker(lines[titleIndex]))
    ) {
      index += 1;
      continue;
    }

    tracks.push({
      number: marker.number,
      title: clean(title),
      artist: clean(artist),
    });
    index = artistIndex + 1;
  }

  return tracks;
}

function parseUnnumberedMetadataBlocks(lines: string[]): TrackMetadata[] {
  const tracks: TrackMetadata[] = [];
  let index = 0;

  while (index + 1 < lines.length) {
    const title = lines[index];
    if (isMetadataLine(title) || parseNumberMarker(title)) {
      index += 1;
      continue;
    }

    let metadataEnd = index + 1;
    while (metadataEnd < lines.length && isMetadataLine(lines[metadataEnd])) {
      metadataEnd += 1;
    }

    if (metadataEnd === index + 1) {
      index += 1;
      continue;
    }

    const metadataLines = lines.slice(index + 1, metadataEnd);
    tracks.push({
      number: tracks.length + 1,
      title: clean(title),
      artist: extractArtistFromBlock(metadataLines),
    });
    index = metadataEnd;
  }

  return tracks;
}

function parseNumberMarker(line: string): { number: number; title?: string } | null {
  const match = line.match(/^(?:tr(?:ack)?\.?\s*)?(\d{1,3})(?:[\.\)\]:\-\s\t]+(.*?))?$/i);
  if (!match) return null;

  return {
    number: Number(match[1]),
    title: match[2] ? clean(match[2]) : undefined,
  };
}

function isMetadataLine(line: string): boolean {
  const normalized = line.trim().toLowerCase();
  return (
    normalized.startsWith("lyrics") ||
    normalized.startsWith("music") ||
    normalized.startsWith("compose") ||
    normalized.startsWith("arrange") ||
    normalized.startsWith("from:") ||
    normalized.startsWith("from：")
  );
}

function extractArtistFromBlock(lines: string[]): string {
  for (const line of lines) {
    const trimmed = line.trim();
    const lower = trimmed.toLowerCase();
    if (!lower.startsWith("from:") && !lower.startsWith("from：")) continue;

    const slashIndex = findFirstIndex(trimmed, ["/", "／"]);
    if (slashIndex >= 0) return clean(trimmed.slice(slashIndex + 1));
  }

  const creditLine = lines.find(
    (line) => isMetadataLine(line) && !line.trim().toLowerCase().startsWith("from"),
  );
  return creditLine ? extractCreditFromMetadataLine(creditLine) : "";
}

function extractCreditFromMetadataLine(line: string): string {
  return (
    extractLabeledCredit(line, ["lyrics", "lyric"]) ||
    extractLabeledCredit(line, ["music", "compose", "composition"]) ||
    extractLabeledCredit(line, ["arrangement", "arrange"]) ||
    clean(line.split(/[:：]/).filter(Boolean).pop() || "")
  );
}

function extractLabeledCredit(line: string, labels: string[]): string {
  const lower = line.toLowerCase();
  const allLabels = ["lyrics", "lyric", "music", "compose", "composition", "arrangement", "arrange"];

  for (const label of labels) {
    const labelStart = lower.indexOf(label);
    if (labelStart < 0) continue;

    const valueStart = labelStart + label.length;
    const valueEnd =
      allLabels
        .map((nextLabel) => {
          const nextIndex = lower.slice(valueStart).indexOf(nextLabel);
          return nextIndex >= 0 ? valueStart + nextIndex : -1;
        })
        .filter((nextIndex) => nextIndex >= 0)
        .sort((left, right) => left - right)[0] ?? line.length;
    const value = clean(line.slice(valueStart, valueEnd).replace(/^[\s:：&/]+/, ""));
    if (value) return value;
  }

  return "";
}

function findFirstIndex(text: string, needles: string[]): number {
  return needles
    .map((needle) => text.indexOf(needle))
    .filter((index) => index >= 0)
    .sort((left, right) => left - right)[0] ?? -1;
}

function parseContiguous(text: string): TrackMetadata[] {
  const body = text
    .split("\n")
    .filter((line) => !isHeading(line.trim()))
    .join(" ");
  const chunks: Array<{ number: number; rest: string }> = [];
  let expected = 1;
  let start = body.indexOf(String(expected).padStart(2, "0"));

  if (start < 0) return [];

  while (start >= 0) {
    const marker = String(expected).padStart(2, "0");
    const contentStart = start + marker.length;
    const nextMarker = String(expected + 1).padStart(2, "0");
    const nextStart = body.indexOf(nextMarker, contentStart);
    const rest = body
      .slice(contentStart, nextStart < 0 ? undefined : nextStart)
      .trim()
      .replace(/^[.)\]:\-\s]+/, "");

    if (rest) {
      chunks.push({ number: expected, rest });
    }

    if (nextStart < 0) break;
    start = nextStart;
    expected += 1;
  }

  const seed = inferArtistSeed(chunks.map((chunk) => chunk.rest));
  return chunks
    .map((chunk) => {
      const parsed = parseRest(chunk.rest, seed);
      return parsed ? { number: chunk.number, ...parsed } : null;
    })
    .filter((track): track is TrackMetadata => Boolean(track));
}

function parseRest(rest: string, seed?: string): { title: string; artist: string } | null {
  const slash = splitBy(rest, [" / ", " ／ ", "\t/\t", "\t／\t"]);
  if (slash) return { title: clean(slash[0]), artist: clean(slash[1]) };

  const metadataSuffix = splitMetadataSuffix(rest);
  if (metadataSuffix) return metadataSuffix;

  const dash = splitBy(rest, ["\t-\t", " - ", " – ", " — "]);
  if (dash) return { title: clean(dash[1]), artist: clean(dash[0]) };

  const columns = splitColumns(rest);
  if (columns) return { title: clean(columns[0]), artist: clean(columns.slice(1).join(" ")) };

  if (seed) {
    const index = rest.lastIndexOf(seed);
    if (index > 0) {
      return { title: clean(rest.slice(0, index)), artist: clean(rest.slice(index)) };
    }
  }

  const markerIndex = findArtistMarker(rest);
  if (markerIndex < 0) return null;

  const prefix = rest.slice(0, markerIndex);
  const boundary = findAsciiBoundary(prefix) ?? prefix.lastIndexOf(" ") + 1;
  if (boundary <= 0) return null;

  return { title: clean(rest.slice(0, boundary)), artist: clean(rest.slice(boundary)) };
}

function splitMetadataSuffix(rest: string): { title: string; artist: string } | null {
  const split = splitBy(rest, [" - ", " – ", " — ", "\t-\t"]);
  if (!split || !isMetadataLine(split[1])) return null;

  const title = clean(split[0]);
  const artist = extractCreditFromMetadataLine(split[1]);
  return title && artist ? { title, artist } : null;
}

function splitColumns(text: string): string[] | null {
  if (text.includes("\t")) {
    const columns = text
      .split("\t")
      .map((column) => column.trim())
      .filter(Boolean);

    return columns.length >= 2 ? columns : null;
  }

  const columns = text
    .split(/\s{2,}/)
    .map((column) => column.trim())
    .filter(Boolean);

  return columns.length >= 2 && findArtistMarker(columns[1]) >= 0 ? columns : null;
}

function splitBy(text: string, separators: string[]): [string, string] | null {
  for (const separator of separators) {
    const index = text.indexOf(separator);
    if (index > 0) {
      const left = text.slice(0, index).trim();
      const right = text.slice(index + separator.length).trim();
      if (left && right) return [left, right];
    }
  }

  return null;
}

function inferArtistSeed(rests: string[]): string | undefined {
  const prefixes = rests
    .map((rest) => {
      const marker = findArtistMarker(rest);
      return marker >= 0 ? rest.slice(0, marker) : "";
    })
    .filter(Boolean);

  if (prefixes.length < 2) return undefined;

  let suffix = "";
  const minLength = Math.min(...prefixes.map((prefix) => prefix.length));
  for (let index = 1; index <= minLength; index += 1) {
    const char = prefixes[0][prefixes[0].length - index];
    if (prefixes.every((prefix) => prefix[prefix.length - index] === char)) {
      suffix = `${char}${suffix}`;
    } else {
      break;
    }
  }

  return suffix.trim() || undefined;
}

function findArtistMarker(text: string): number {
  const lower = text.toLowerCase();
  return [" feat.", " feat ", " ft.", " featuring ", " vocal", " vo."]
    .map((marker) => lower.indexOf(marker))
    .filter((index) => index >= 0)
    .sort((left, right) => left - right)[0] ?? -1;
}

function findAsciiBoundary(text: string): number | null {
  for (let index = 1; index < text.length; index += 1) {
    const previous = text[index - 1];
    const current = text[index];
    if (!/[\x00-\x7F]/.test(previous) && /[A-Za-z0-9]/.test(current)) {
      return index;
    }
  }

  return null;
}

function clean(value: string): string {
  return value.trim().replace(/^[-/／\s]+|[-/／\s]+$/g, "");
}

function isHeading(line: string): boolean {
  return ["tracklist", "track list", "トラックリスト", "曲目", "曲目リスト", "曲目列表"].includes(
    line.toLowerCase(),
  );
}
