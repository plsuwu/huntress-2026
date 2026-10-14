#!/usr/bin/env python3
"""Mastery: deliveryN.zip files -> answers keyed 'setN.<id>' (N = delivery order, matches question ids set1./set2.).
usage: mastery.py <run_dir>   (run_dir has delivery1.zip, ...) -> <run_dir>/answers.json
Archives are AES/`infected`-encrypted, which python zipfile cannot read, so 7z extracts them (nested zips are
unpacked recursively; the dir that holds the d01-- specimen is solved as the question set with solve.py)."""
import sys, json, subprocess, tempfile
from pathlib import Path

run = Path(sys.argv[1]); here = Path(__file__).resolve().parent

def extract(z, dest):
    subprocess.run(['7z', 'x', '-y', '-pinfected', f'-o{dest}', str(z)], capture_output=True, check=True)
    for inner in list(dest.rglob('*.zip')):
        extract(inner, inner.with_suffix(''))

ans = {}
for z in sorted(run.glob('delivery*.zip'), key=lambda p: int(p.stem[8:])):
    n = int(z.stem[8:])
    with tempfile.TemporaryDirectory() as t:
        extract(z, Path(t))
        d01 = next(Path(t).rglob('d01--*'))
        out = subprocess.run([sys.executable, '-I', str(here / 'solve.py'), str(d01.parent)],
                             capture_output=True, text=True, check=True).stdout
    for k, v in json.loads(out).items():
        ans[f'set{n}.{k}'] = v
(run / 'answers.json').write_text(json.dumps(ans, indent=1))
print(len(ans), 'answers')
