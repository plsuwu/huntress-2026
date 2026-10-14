#!/usr/bin/env python3
"""Go Fish (day 04, binary RE) single_solve. Usage: solve.py <evidence_dir> [start.json]
Prints answers JSON keyed by answer id; derives everything from the delivered bytes."""
import sys, json, hashlib, struct, zlib, re, subprocess
from pathlib import Path

ev = Path(sys.argv[1])
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
A = {}

def elf_sections(d):
    shoff, = struct.unpack_from('<Q', d, 0x28)
    shentsize, shnum, shstr = struct.unpack_from('<HHH', d, 0x3a)
    secs = []
    for i in range(shnum):
        nm, ty, fl, ad, off, sz, lk, inf, al, es = struct.unpack_from('<IIQQQQIIQQ', d, shoff + i * shentsize)
        secs.append(dict(nm=nm, ty=ty, off=off, sz=sz, lk=lk, es=es))
    so = secs[shstr]['off']
    for s in secs:
        e = d.index(b'\0', so + s['nm']); s['name'] = d[so + s['nm']:e].decode()
    return secs

def cstr(d, o):
    return d[o:d.index(b'\0', o)].decode()

def hcfg(d):
    """HCFG blob: magic, ver, xor key, len(LE16), crc32(LE) of plaintext, xor'd JSON."""
    i = d.find(b'HCFG')
    key = d[i + 5]; n = int.from_bytes(d[i + 6:i + 8], 'little')
    crc = int.from_bytes(d[i + 8:i + 12], 'little')
    p = bytes(b ^ key for b in d[i + 12:i + 12 + n])
    assert zlib.crc32(p) == crc
    return json.loads(p), key, crc

def pick(day):
    # evidence variants differ: dNN specimen may be .elf or .exe (PE); skip config/sidecar files
    return next(p for p in sorted(ev.glob(day + '--*')) if p.suffix not in ('.json',))

def machine(d):
    if d[:4] == b'\x7fELF':
        return MACH.get(struct.unpack_from('<H', d, 18)[0], 'unknown')
    pe = struct.unpack_from('<I', d, 0x3c)[0]
    return PEMACH.get(struct.unpack_from('<H', d, pe + 4)[0], 'unknown')

PEMACH = {0x8664: 'x86-64', 0x14c: 'x86', 0xaa64: 'aarch64', 0x1c4: 'arm'}
MACH = {62: 'x86-64', 3: 'x86', 183: 'aarch64', 40: 'arm'}

# d01
d = (pick('d01')).read_bytes()
A['d01_artifact_sha256_1'] = sha(pick('d01'))
A['d01_analysis_elf_class'] = {1: 'ELF32', 2: 'ELF64'}[d[4]]
A['d01_analysis_machine'] = MACH[struct.unpack_from('<H', d, 18)[0]]
A['d01_analysis_endianness'] = {1: 'little', 2: 'big'}[d[5]]
A['d01_analysis_elf_type'] = {2: 'EXEC', 3: 'DYN', 1: 'REL'}[struct.unpack_from('<H', d, 16)[0]]
secs = elf_sections(d)
A['d01_analysis_section_names'] = json.dumps([s['name'] for s in secs], separators=(',', ':'))
sym = next(s for s in secs if s['ty'] == 2); strt = secs[sym['lk']]
syms = []
for o in range(sym['off'], sym['off'] + sym['sz'], 24):
    nm, info, oth, shn, val, size = struct.unpack_from('<IBBHQQ', d, o)
    syms.append((cstr(d, strt['off'] + nm), info & 15, val, size))
A['d01_analysis_symbols'] = json.dumps([s[0] for s in syms if s[0]], separators=(',', ':'))
# recover_code is a tiny constant-return function: decode with objdump (static tool, not running the binary)
out = subprocess.run(['objdump', '-d', '-M', 'intel', '--no-show-raw-insn', '--disassemble=recover_code',
                      str(pick('d01'))], capture_output=True, text=True).stdout
ins = [re.sub(r'^\s*[0-9a-f]+:\s*', '', l).strip() for l in out.splitlines() if re.match(r'^\s*[0-9a-f]+:\s', l)]
A['d01_analysis_disassembly'] = json.dumps([re.sub(r'\s+', ' ', x).replace('ret', 'ret') for x in ins], separators=(',', ':'))

# d02
p = pick('d02'); d = p.read_bytes()
A['d02_artifact_sha256_1'] = sha(p)
A['d02_analysis_format'] = 'ELF' if d[:4] == b'\x7fELF' else 'PE'
c, key, crc = hcfg(d)
for k in ('campaign', 'domain', 'mutex', 'url'):
    A['d02_analysis_config_' + k] = c[k]
A['d02_analysis_config_crc32_hex'] = '0x%08x' % crc
A['d02_analysis_xor_key_hex'] = '0x%02x' % key

# d03
p = next(ev.glob('d03--*.dll')); d = p.read_bytes()
A['d03_artifact_sha256_1'] = sha(p)
A['d03_artifact_sha256_2'] = sha(next(ev.glob('d03--*.runtimeconfig.json')))
pe = struct.unpack_from('<I', d, 0x3c)[0]
A['d03_analysis_format'] = 'PE32_dotnet'
A['d03_analysis_machine'] = MACH.get(struct.unpack_from('<H', d, pe + 4)[0], 'x86')
i = d.find(b'BSJB'); vl = struct.unpack_from('<I', d, i + 12)[0]
A['d03_analysis_metadata_version'] = d[i + 16:i + 16 + vl].rstrip(b'\0').decode()
# config blob is XOR'd with a single byte; locate it by brute-forcing the key against '{"campaign"'
for k in range(256):
    j = d.find(bytes(b ^ k for b in b'{"campaign"'))
    if j >= 0:
        break
txt = bytes(b ^ k for b in d[j:j + 600]).decode('latin1')
c3 = json.JSONDecoder().raw_decode(txt)[0]
for k2 in ('campaign', 'domain', 'mutex'):
    A['d03_analysis_config_' + k2] = c3[k2]

# d04
p = pick('d04'); d = p.read_bytes()
A['d04_artifact_sha256_1'] = sha(p)
A['d04_analysis_architecture'] = machine(d)
A['d04_analysis_format'] = 'ELF' if d[:4] == b'\x7fELF' else 'PE'
c, key, crc = hcfg(d)
for k in ('campaign', 'domain', 'mutex', 'url'):
    A['d04_analysis_config_' + k] = c[k]
A['d04_analysis_config_crc32_hex'] = '0x%08x' % crc

print(json.dumps(A, indent=1))
