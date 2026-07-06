import { describe, expect, it } from "vitest";
import { bitAt, indexOfBit, isSet, popcount } from "./bits";

describe("bitAt / isSet", () => {
  it.each([0, 27, 28, 35, 36, 63])(
    "round-trips a single bit at index %i",
    (index) => {
      const mask = bitAt(index);
      expect(isSet(mask, index)).toBe(true);
      expect(isSet(mask, (index + 1) % 64)).toBe(false);
    },
  );
});

describe("indexOfBit", () => {
  it.each([0, 27, 28, 35, 36, 63])(
    "recovers index %i from a single-bit mask",
    (index) => {
      expect(indexOfBit(bitAt(index))).toBe(index);
    },
  );
});

describe("popcount", () => {
  it("is 0 for an empty mask", () => {
    expect(popcount(0n)).toBe(0);
  });

  it("counts the 4 starting disks", () => {
    const mask = bitAt(27) | bitAt(28) | bitAt(35) | bitAt(36);
    expect(popcount(mask)).toBe(4);
  });

  it("counts all 64 bits set", () => {
    expect(popcount((1n << 64n) - 1n)).toBe(64);
  });
});
