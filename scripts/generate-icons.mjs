// Generates placeholder icons for Tauri bundle.
// PNGs are generated from scratch using Node's built-in zlib.
// ICO wraps a PNG in Windows icon format; ICNS wraps a PNG in macOS icon format.

import zlib from "node:zlib";
import fs from "node:fs";
import path from "node:path";

const OUT = path.resolve("src-tauri/icons");

/* ── helpers ─────────────────────────────────────── */

function crc32(buf) {
  // CRC-32 per PNG spec
  let crc = 0xffffffff;
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c;
  }
  for (let i = 0; i < buf.length; i++) crc = table[(crc ^ buf[i]) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const typeB = Buffer.from(type, "ascii");
  const crcBuf = Buffer.concat([typeB, data]);
  const crcV = Buffer.alloc(4);
  crcV.writeUInt32BE(crc32(crcBuf));
  return Buffer.concat([len, typeB, data, crcV]);
}

function makePNG(width, height, r, g, b) {
  // 1. Signature
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

  // 2. IHDR
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;   // bit depth
  ihdr[9] = 2;   // color type: RGB (no alpha for simplicity)
  ihdr[10] = 0;  // compression
  ihdr[11] = 0;  // filter
  ihdr[12] = 0;  // interlace
  const ihdrChunk = pngChunk("IHDR", ihdr);

  // 3. IDAT – raw scanlines (filter byte 0x00 then RGB pixels)
  const raw = Buffer.alloc(height * (1 + width * 3));
  for (let y = 0; y < height; y++) {
    const off = y * (1 + width * 3);
    raw[off] = 0; // filter none
    for (let x = 0; x < width; x++) {
      const p = off + 1 + x * 3;
      raw[p] = r;
      raw[p + 1] = g;
      raw[p + 2] = b;
    }
  }
  const deflated = zlib.deflateSync(raw);
  const idatChunk = pngChunk("IDAT", deflated);

  // 4. IEND
  const iendChunk = pngChunk("IEND", Buffer.alloc(0));

  return Buffer.concat([sig, ihdrChunk, idatChunk, iendChunk]);
}

function makeICO(png32) {
  // ICO header: reserved(2), type=1(2), count(2)
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);    // reserved
  header.writeUInt16LE(1, 2);    // type: 1 = icon
  header.writeUInt16LE(1, 4);    // count: 1 image

  // Directory entry: w, h, colors, reserved, planes, bpp, size, offset
  const entry = Buffer.alloc(16);
  entry[0] = 32;                 // width (32)
  entry[1] = 32;                 // height (32)
  entry[2] = 0;                  // colors
  entry[3] = 0;                  // reserved
  entry.writeUInt16LE(1, 4);     // color planes
  entry.writeUInt16LE(32, 6);    // bits per pixel
  entry.writeUInt32LE(png32.length, 8);  // image size
  entry.writeUInt32LE(22, 12);   // offset (6 header + 16 entry = 22)

  return Buffer.concat([header, entry, png32]);
}

function makeICNS(png128) {
  // ICNS container: 'icns' + total_size + icon_entry
  // Icon entry: type 'ic07' (128x128 PNG) + size + data
  const iconType = Buffer.from("ic07", "ascii");
  const iconSize = Buffer.alloc(4);
  iconSize.writeUInt32BE(8 + png128.length); // type(4) + size(4) + data
  const iconEntry = Buffer.concat([iconType, iconSize, png128]);

  const header = Buffer.from("icns", "ascii");
  const totalSize = Buffer.alloc(4);
  totalSize.writeUInt32BE(8 + iconEntry.length);
  return Buffer.concat([header, totalSize, iconEntry]);
}

/* ── generate ────────────────────────────────────── */

fs.mkdirSync(OUT, { recursive: true });

// Use a gradient-inspired blue-purple palette (matching the app title gradient)
const png32 = makePNG(32, 32, 88, 166, 255);    // #58a6ff accent blue
const png128 = makePNG(128, 128, 88, 166, 255);
const png256 = makePNG(256, 256, 88, 166, 255);

fs.writeFileSync(path.join(OUT, "32x32.png"), png32);
fs.writeFileSync(path.join(OUT, "128x128.png"), png128);
fs.writeFileSync(path.join(OUT, "128x128@2x.png"), png256);
fs.writeFileSync(path.join(OUT, "icon.ico"), makeICO(png32));
fs.writeFileSync(path.join(OUT, "icon.icns"), makeICNS(png128));

console.log("Icons generated in", OUT);
