export function bitAt(index: number): bigint {
  return 1n << BigInt(index);
}

export function isSet(mask: bigint, index: number): boolean {
  return ((mask >> BigInt(index)) & 1n) === 1n;
}

// Cell index of a single-bit mask.
export function indexOfBit(bit: bigint): number {
  return bit.toString(2).length - 1;
}

export function popcount(x: bigint): number {
  let c = 0;
  while (x > 0n) {
    x &= x - 1n;
    c++;
  }
  return c;
}
