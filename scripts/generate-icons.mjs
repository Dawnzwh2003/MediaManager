// Small deterministic, code-native app icon. No external image tooling required.
import { mkdir, writeFile } from "node:fs/promises";
import { deflateSync } from "node:zlib";

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function chunk(name, data) {
  const type = Buffer.from(name);
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const checksum = Buffer.alloc(4);
  checksum.writeUInt32BE(crc32(Buffer.concat([type, data])));
  return Buffer.concat([length, type, data, checksum]);
}
const size = 32;
const pixels = Buffer.alloc(size * (1 + size * 4));
for (let y = 0; y < size; y++) {
  for (let x = 0; x < size; x++) {
    const mark = x >= 8 && x <= 23 && y >= 8 && y <= 23 &&
      (x <= 10 || x >= 21 || (y <= 16 && Math.abs(x - 15.5) <= 16 - y));
    const offset = y * (1 + size * 4) + 1 + x * 4;
    pixels.set(mark ? [239, 246, 233, 255] : [36, 79, 58, 255], offset);
  }
}
const header = Buffer.alloc(13);
header.writeUInt32BE(size, 0);
header.writeUInt32BE(size, 4);
header[8] = 8;
header[9] = 6;
const png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", header), chunk("IDAT", deflateSync(pixels)), chunk("IEND", Buffer.alloc(0)),
]);
const ico = Buffer.alloc(22);
ico.writeUInt16LE(1, 2);
ico.writeUInt16LE(1, 4);
ico[6] = size;
ico[7] = size;
ico.writeUInt16LE(1, 10);
ico.writeUInt16LE(32, 12);
ico.writeUInt32LE(png.length, 14);
ico.writeUInt32LE(22, 18);
const directory = new URL("../src-tauri/icons/", import.meta.url);
await mkdir(directory, { recursive: true });
await writeFile(new URL("icon.png", directory), png);
await writeFile(new URL("icon.ico", directory), Buffer.concat([ico, png]));
