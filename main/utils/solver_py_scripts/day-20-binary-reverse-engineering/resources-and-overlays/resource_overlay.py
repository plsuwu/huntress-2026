"""Day 20 binary RE: decode the final validated RS20 resource overlay of a specimen and its HCFG record.

RS20 layout (from the question text):
  +0 "RS20" | +8 resource_len u32le | +12 scratch_len u16le | +16 sha256(scratch)[32] | +48 resource | scratch
The overlay must end exactly at EOF; the resource holds a plain HCFG record at offset 0.

Usage: python3 -I resource_overlay.py <specimen>
"""
import hashlib
import json
import struct
import sys
import zlib


def rs_overlays(d: bytes, magic: bytes = b"RS20"):
    i = d.rfind(magic)
    while i >= 0:
        n, s = struct.unpack_from("<IH", d, i + 8)
        end = i + 48 + n + s
        if end == len(d) and hashlib.sha256(d[i + 48 + n:end]).digest() == d[i + 16:i + 48]:
            yield i, d[i + 48:i + 48 + n]
        i = d.rfind(magic, 0, i)


def hcfg(r: bytes):
    assert r[:4] == b"HCFG"
    ver, key, ln, crc = r[4], r[5], *struct.unpack_from("<HI", r, 6)
    payload = bytes(b ^ key for b in r[12:12 + ln])
    assert zlib.crc32(payload) == crc
    return {"version": ver, "xor_key": f"0x{key:02x}", "crc32": f"0x{crc:08x}", "config": json.loads(payload)}


if __name__ == "__main__":
    data = open(sys.argv[1], "rb").read()
    off, res = next(rs_overlays(data))
    print(json.dumps({"overlay_offset": off, "overlay_size": len(data) - off, "hcfg_resource_size": len(res), **hcfg(res)}, indent=1))
