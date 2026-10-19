#!/usr/bin/env python3
"""Unpack the final PK<dd> overlay of a d19 specimen and decode its HCFG config.

  python3 -I unpack_overlay.py <d19--variant-NNN.elf>
"""
import hashlib, json, re, struct, sys, zlib


def unpack(d):
    for m in reversed([m.start() for m in re.finditer(rb"PK\d\d", d)]):
        k1, k2 = d[m + 5], d[m + 6]
        n = struct.unpack_from("<I", d, m + 8)[0]
        want, packed = d[m + 12:m + 44], d[m + 44:m + 44 + n]
        for r in range(8):
            u = bytes((((b ^ k2) >> r) | ((b ^ k2) << (8 - r))) & 0xFF ^ k1 for b in packed)
            if hashlib.sha256(u).digest() == want:
                return m, k1, k2, r, u
    return None


def hcfg(u, i=0):
    assert u[i:i + 4] == b"HCFG"
    key, n, crc = u[i + 5], *struct.unpack_from("<HI", u, i + 6)
    plain = bytes(b ^ key for b in u[i + 12:i + 12 + n])
    assert zlib.crc32(plain) == crc
    return u[i + 4], key, crc, json.loads(plain)


if __name__ == "__main__":
    d = open(sys.argv[1], "rb").read()
    off, k1, k2, r, u = unpack(d)
    ver, key, crc, cfg = hcfg(u)
    print(json.dumps({"overlay_offset": off, "overlay_size": len(d) - off, "k1": k1, "k2": k2, "ror": r,
                      "hcfg_version": ver, "xor_key": f"0x{key:02x}", "crc32": f"0x{crc:08x}", "config": cfg}, indent=1))
