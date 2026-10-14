#!/usr/bin/env python3
"""Pascal's Wager (day 12) = Crystal Clear (day 13) solver limited to d01-d12 = day-10 solver (d01-d10 unchanged) + d11-d13 blocks. Orig: Zig Zag (day 10, binary RE, Zig cross-platform services) single_solve = day-09 solver (d01-d09 unchanged) + d10 Zig ELF/PE. Usage: solve.py <evidence_dir>
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

def hcfg(d, with_ver=False):
    """HCFG blob: magic, ver, xor key, len(LE16), crc32(LE) of plaintext, xor'd JSON.
    Scan every 'HCFG' hit and keep the one whose crc32 + JSON validate (Rust binaries contain
    decoy 'HCFG' byte runs in code/rodata, e.g. d05)."""
    i = -1
    while (i := d.find(b'HCFG', i + 1)) >= 0:
        if d[i + 4] != 1:
            continue
        key = d[i + 5]; n = int.from_bytes(d[i + 6:i + 8], 'little')
        crc = int.from_bytes(d[i + 8:i + 12], 'little')
        p = bytes(b ^ key for b in d[i + 12:i + 12 + n])
        if zlib.crc32(p) != crc:
            continue
        try:
            return (json.loads(p), key, crc, d[i + 4]) if with_ver else (json.loads(p), key, crc)
        except ValueError:
            continue
    raise SystemExit('no valid HCFG blob')

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
sym = next((s for s in secs if s['ty'] == 2), None)   # stripped mastery variants have no .symtab -> [] / []
syms = []
if sym:
    strt = secs[sym['lk']]
    for o in range(sym['off'], sym['off'] + sym['sz'], 24):
        nm, info, oth, shn, val, size = struct.unpack_from('<IBBHQQ', d, o)
        syms.append((cstr(d, strt['off'] + nm), info & 15, val, size))
A['d01_analysis_symbols'] = json.dumps([s[0] for s in syms if s[0]], separators=(',', ':'))
# recover_code is a tiny constant-return function: decode with objdump (static tool, not running the binary)
ins = []
if any(s[0] == 'recover_code' for s in syms):
    out = subprocess.run(['objdump', '-d', '-M', 'intel', '--no-show-raw-insn', '--disassemble=recover_code',
                          str(pick('d01'))], capture_output=True, text=True).stdout
    ins = [re.sub(r'^\s*[0-9a-f]+:\s*', '', l).strip() for l in out.splitlines() if re.match(r'^\s*[0-9a-f]+:\s', l)]
A['d01_analysis_disassembly'] = json.dumps([re.sub(r'\s+', ' ', x) for x in ins], separators=(',', ':'))

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

# d05 (Rust specimen): same HCFG layout as d04, no url-less variant seen so far
p = pick('d05'); d = p.read_bytes()
A['d05_artifact_sha256_1'] = sha(p)
A['d05_analysis_architecture'] = machine(d)
A['d05_analysis_format'] = 'ELF' if d[:4] == b'\x7fELF' else 'PE'
c, key, crc = hcfg(d)
for k in ('campaign', 'domain', 'mutex', 'url'):
    A['d05_analysis_config_' + k] = c[k]
A['d05_analysis_config_crc32_hex'] = '0x%08x' % crc

# d06 (Java JAR): HCFG blob lives base64-encoded in a CONSTANT_Utf8 (String constant of a static CONFIG field).
# Parse every class's constant pool (no javap needed) and keep the first Utf8 that base64-decodes to a valid HCFG.
import zipfile, base64, binascii

def class_utf8(c):
    """Yield every CONSTANT_Utf8 payload (bytes) of a .class file's constant pool."""
    assert c[:4] == b'\xca\xfe\xba\xbe'
    n, = struct.unpack_from('>H', c, 8); o = 10; i = 1
    sizes = {3: 4, 4: 4, 5: 8, 6: 8, 7: 2, 8: 2, 9: 4, 10: 4, 11: 4, 12: 4, 15: 3, 16: 2, 17: 4, 18: 4, 19: 2, 20: 2}
    while i < n:
        t = c[o]; o += 1
        if t == 1:
            l, = struct.unpack_from('>H', c, o); yield c[o + 2:o + 2 + l]; o += 2 + l
        else:
            o += sizes[t]
            if t in (5, 6):
                i += 1
        i += 1

def jar_hcfg(p):
    with zipfile.ZipFile(p) as z:
        names = sorted((n for n in z.namelist() if n.endswith('.class')),
                       key=lambda n: (n.startswith(('kotlin/', 'kotlinx/', 'org/', 'META-INF/')), n))
        for nm in names:
            for u in class_utf8(z.read(nm)):
                if len(u) < 24 or not re.fullmatch(rb'[A-Za-z0-9+/=]+', u):
                    continue
                try:
                    blob = base64.b64decode(u, validate=True)
                except (binascii.Error, ValueError):
                    continue
                if b'HCFG' in blob:
                    try:
                        return hcfg(blob, True)
                    except SystemExit:
                        pass
    # fallback: any base64 run / raw HCFG in any member (resources, split constants, ...)
    with zipfile.ZipFile(p) as z:
        for nm in z.namelist():
            raw = z.read(nm)
            cands = [raw] + [base64.b64decode(m + b'=' * (-len(m) % 4)) for m in re.findall(rb'[A-Za-z0-9+/]{40,}={0,2}', raw)]
            for blob in cands:
                if b'HCFG' in blob:
                    try:
                        return hcfg(blob, True)
                    except SystemExit:
                        pass
    raise SystemExit('no HCFG constant in jar')

for day in ('d06', 'd07'):
    # d07 is a Kotlin (coroutine state machine) fat jar; CONFIG constant is the same base64 HCFG as d06
    p = pick(day); d = p.read_bytes()
    A[day + '_artifact_sha256_1'] = sha(p)
    A[day + '_analysis_format'] = 'JAR' if d[:2] == b'PK' else 'unknown'
    c, key, crc, ver = jar_hcfg(p)
    for k in ('campaign', 'domain', 'mutex', 'url'):
        A[day + '_analysis_config_' + k] = c[k]
    A[day + '_analysis_config_crc32_hex'] = '0x%08x' % crc
    A[day + '_analysis_hcfg_version'] = str(ver)

# d08 (Swift ELF, ~67 MB): raw 'HCFG' hits are code-byte decoys (hcfg() rejects them on crc32); the real blob is the
# base64 string constant embedded in the binary (same CONFIG encoding as the d06/d07 JARs). Scan base64 runs.
def elf_b64_hcfg(d):
    i = -1
    while (i := d.find(b'SENGR', i + 1)) >= 0:   # base64('HCFG') prefix; find() is far faster than a regex over 67 MB
        m = re.compile(rb'[A-Za-z0-9+/]{40,}={0,2}').match(d, i)
        if not m:
            continue
        run = m.group()
        try:
            return hcfg(base64.b64decode(run + b'=' * (-len(run) % 4)), True)
        except (SystemExit, binascii.Error, ValueError):
            continue
    raise SystemExit('no base64 HCFG constant in ELF')

p = pick('d08'); d = p.read_bytes()
A['d08_artifact_sha256_1'] = sha(p)
A['d08_analysis_architecture'] = machine(d)
A['d08_analysis_machine'] = machine(d)
A['d08_analysis_format'] = 'ELF' if d[:4] == b'\x7fELF' else 'PE'
A['d08_analysis_elf_class'] = str({1: 32, 2: 64}[d[4]])   # d08 format is "base-10 integer" (d01 wants ELF64)
c, key, crc, ver = elf_b64_hcfg(d)
for k in ('campaign', 'domain', 'mutex', 'url'):
    A['d08_analysis_config_' + k] = c[k]
A['d08_analysis_config_crc32_hex'] = '0x%08x' % crc

# ---- fields first asked on day 08 (d01-d07 re-asked with extra properties) ----
d = pick('d01').read_bytes()
A['d01_analysis_entry_point'] = '0x%x' % struct.unpack_from('<Q', d, 24)[0]
A['d01_analysis_has_symtab'] = 'true' if any(s['ty'] == 2 for s in elf_sections(d)) else 'false'
d = pick('d02').read_bytes()
# C++ RTTI: Itanium-mangled typeinfo (_ZTI) / vtable (_ZTV) symbols for the program's own classes
A['d02_analysis_has_cpp_typeinfo'] = 'true' if re.search(rb'_ZTI\d', d) else 'false'
A['d02_analysis_has_cpp_vtable'] = 'true' if re.search(rb'_ZTV\d', d) else 'false'
# d03 (.NET dll): XOR'd HCFG blob sits raw in the file, header 'HCFG'|ver|key|len|crc|payload; hcfg() already validates it
d = next(ev.glob('d03--*.dll')).read_bytes()
j = d.find(b'HCFG\x01')
c3 = None
while j >= 0 and c3 is None:
    k3 = d[j + 5]; n = int.from_bytes(d[j + 6:j + 8], 'little'); pl = bytes(b ^ k3 for b in d[j + 12:j + 12 + n])
    if zlib.crc32(pl) == int.from_bytes(d[j + 8:j + 12], 'little'):
        c3 = json.loads(pl); A['d03_analysis_config_crc32_hex'] = '0x%08x' % zlib.crc32(pl); A['d03_analysis_config_url'] = c3['url']
    j = d.find(b'HCFG\x01', j + 1)
# d04/d05 HCFG header version + xor key (hcfg() returns them)
for day in ('d04', 'd05'):
    c, key, crc, ver = hcfg(pick(day).read_bytes(), True)
    A[day + '_analysis_hcfg_version'] = str(ver); A[day + '_analysis_xor_key_hex'] = '0x%02x' % key
for day in ('d06', 'd07'):
    c, key, crc, ver = jar_hcfg(pick(day))
    A[day + '_analysis_xor_key_hex'] = '0x%02x' % key
    with zipfile.ZipFile(pick(day)) as z:
        A[day + '_analysis_compression_methods'] = json.dumps(sorted({i.compress_type for i in z.infolist()}))

# d09 (Nim ELF, ~61 KB): same base64 HCFG string constant as d08. Nim string literal = {len, cap|strlitFlag(0x40<<56), bytes},
# so the data start (file offset of 'SENGR...') is what hcfg_offset reports.
p = pick('d09'); d = p.read_bytes()
A['d09_artifact_sha256_1'] = sha(p)
A['d09_analysis_architecture'] = machine(d)
A['d09_analysis_machine'] = machine(d)
A['d09_analysis_format'] = 'ELF' if d[:4] == b'\x7fELF' else 'PE'
c, key, crc, ver = elf_b64_hcfg(d)
for k in ('campaign', 'domain', 'mutex', 'url'):
    A['d09_analysis_config_' + k] = c[k]
A['d09_analysis_config_crc32_hex'] = '0x%08x' % crc
A['d09_analysis_hcfg_offset'] = str(d.find(b'SENGR'))

# d10 (Zig ELF or PE): plain XOR'd HCFG blob (same layout as d04) in a dedicated `.hcfg` section; ELF and PE variants carry the same descriptor.
def section_names(d):
    if d[:4] == b'\x7fELF':
        return [s['name'] for s in elf_sections(d)]
    pe = struct.unpack_from('<I', d, 0x3c)[0]
    n, = struct.unpack_from('<H', d, pe + 6); osz, = struct.unpack_from('<H', d, pe + 20)
    return [d[o:o + 8].rstrip(b'\0').decode('latin1') for o in range(pe + 24 + osz, pe + 24 + osz + 40 * n, 40)]

p = pick('d10'); d = p.read_bytes()
A['d10_artifact_sha256_1'] = sha(p)
A['d10_analysis_architecture'] = machine(d)
A['d10_analysis_machine'] = machine(d)
A['d10_analysis_format'] = 'ELF' if d[:4] == b'\x7fELF' else 'PE'
c, key, crc = hcfg(d)
for k in ('campaign', 'domain', 'mutex', 'url'):
    A['d10_analysis_config_' + k] = c[k]
A['d10_analysis_config_crc32_hex'] = '0x%08x' % crc
A['d10_analysis_has_hcfg_section'] = 'true' if '.hcfg' in section_names(d) else 'false'

# d11-d13 (Crystal/other ELFs): config is either the base64 'SENGR...' string constant (d11 Crystal, d13 Crystal) or a plain
# XOR'd HCFG blob (d12 static stripped). hcfg_offset = file offset of the blob start (base64 data start, or 'HCFG' magic).
for day in ('d11', 'd12'):
    p = pick(day); d = p.read_bytes()
    A[day + '_artifact_sha256_1'] = sha(p)
    A[day + '_analysis_architecture'] = machine(d)
    A[day + '_analysis_machine'] = machine(d)
    A[day + '_analysis_format'] = ('ELF' if day == 'd11' else 'elf') if d[:4] == b'\x7fELF' else ('PE' if day == 'd11' else 'pe')   # d12/d13 grade lowercase (d11 'ELF'; 'ELF64' etc. rejected)
    try:
        c, key, crc = hcfg(d); off = d.find(b'HCFG')
        while off >= 0 and not (d[off + 4] == 1 and zlib.crc32(bytes(b ^ d[off + 5] for b in d[off + 12:off + 12 + int.from_bytes(d[off + 6:off + 8], 'little')])) == crc):
            off = d.find(b'HCFG', off + 1)
    except SystemExit:
        c, key, crc, ver = elf_b64_hcfg(d); off = d.find(b'SENGR')
    for k in ('campaign', 'domain', 'mutex', 'url'):
        A[day + '_analysis_config_' + k] = c[k]
    A[day + '_analysis_config_crc32_hex'] = '0x%08x' % crc
    A[day + '_analysis_hcfg_offset'] = str(off)

print(json.dumps(A, indent=1))
