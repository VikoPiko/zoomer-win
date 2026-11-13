// Optional developer utility. The generated icon is committed; Node is not needed to build/run Zoomer.
import { writeFileSync } from 'node:fs';
const bytes = Buffer.alloc(22 + 40 + 32*32*4 + 128);
bytes.writeUInt16LE(1, 2); bytes.writeUInt16LE(1, 4);
bytes[6] = 32; bytes[7] = 32;
bytes.writeUInt16LE(1, 10); bytes.writeUInt16LE(32, 12);
bytes.writeUInt32LE(bytes.length-22, 14); bytes.writeUInt32LE(22, 18);
bytes.writeUInt32LE(40, 22); bytes.writeInt32LE(32, 26); bytes.writeInt32LE(64, 30);
bytes.writeUInt16LE(1, 34); bytes.writeUInt16LE(32, 36); bytes.writeUInt32LE(4096, 42);
let offset = 62;
for (let y=31; y>=0; y--) {
  for (let x=0; x<32; x++) {
    const r = (x-13)**2 + (y-13)**2;
    const glass = (r>=49 && r<=100) || (x>=20 && x<=28 && Math.abs(x-y)<=2 && y>=20);
    bytes.set(glass ? [235,221,89,255] : [43,29,23,255], offset);
    offset += 4;
  }
}
writeFileSync(new URL('../assets/icon.ico', import.meta.url), bytes);
