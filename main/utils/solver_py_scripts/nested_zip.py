"""Reusable nested-zip flattening for Huntress CTF artifacts.

Mastery-mode downloads are usually an outer zip wrapping one inner zip per
question set (e.g. metadata-day-04-evidence-01.zip), and questions reference
files as "<zip name> -> <member>" (sometimes prefixed "delivery 1: ").

    import sys; sys.path.insert(0, "<repo>/utils/solver_py_scripts")  # needed under `python3 -I`
    from nested_zip import load_registry, resolve
    reg = load_registry(zip_bytes)                 # {zipname: {member_basename: bytes}}
    members, name, data = resolve(reg, q["artifact"])

Zips are keyed by basename; members are keyed by basename (directories dropped).
The outer archive is registered as OUTER ("__outer__.zip"). Password-protected
archives are not handled here (password is normally `infected`; use 7z).
"""
import io
import zipfile

OUTER = "__outer__.zip"


def load_registry(data, outer_name=OUTER, _reg=None):
    """Recursively register `data` (zip bytes) and every nested zip."""
    reg = {} if _reg is None else _reg
    members = reg.setdefault(outer_name, {})
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        for n in z.namelist():
            if n.endswith("/"):
                continue
            b = z.read(n)
            base = n.rsplit("/", 1)[-1]
            members[base] = b
            if base.lower().endswith(".zip"):
                load_registry(b, base, reg)
    return reg


def parse_artifact(artifact):
    """'delivery 1: a.zip -> b.jpg' -> ('a.zip', 'b.jpg'); member '' if absent."""
    zname, _, member = (s.strip() for s in artifact.partition("->"))
    return zname.split(": ")[-1], member


def resolve(reg, artifact, fallback_outer=True):
    """Return (members_of_zip, member_name, member_bytes_or_None).
    If the named zip isn't registered (single-solve/time-trial where the download
    itself is the named zip), fall back to the outer archive."""
    zname, member = parse_artifact(artifact)
    members = reg.get(zname) or (reg[OUTER] if fallback_outer else None)
    return members, member, members.get(member) if member else None
