"""Dump a JVM class file: access flags, this/super, fields, methods (name, descriptor, flags)."""
import struct, sys


def parse(b):
    assert b[:4] == b'\xca\xfe\xba\xbe'
    n = struct.unpack('>H', b[8:10])[0]
    cp = [None] * n
    i, k = 10, 1
    while k < n:
        t = b[i]
        if t == 1:
            l = struct.unpack('>H', b[i + 1:i + 3])[0]
            cp[k] = ('utf8', b[i + 3:i + 3 + l].decode('utf-8', 'replace'))
            i += 3 + l
        elif t in (3, 4):
            cp[k] = (t, b[i + 1:i + 5]); i += 5
        elif t in (5, 6):
            cp[k] = (t, b[i + 1:i + 9]); i += 9; k += 1
        elif t in (7, 8, 16, 19, 20):
            cp[k] = (t, struct.unpack('>H', b[i + 1:i + 3])[0]); i += 3
        elif t == 15:
            i += 4
        elif t in (9, 10, 11, 12, 17, 18):
            cp[k] = (t, struct.unpack('>HH', b[i + 1:i + 5])); i += 5
        else:
            raise ValueError(t)
        k += 1
    u = lambda j: cp[j][1]
    acc, this, sup = struct.unpack('>HHH', b[i:i + 6]); i += 6
    ni = struct.unpack('>H', b[i:i + 2])[0]; i += 2 + 2 * ni

    def members(i):
        cnt = struct.unpack('>H', b[i:i + 2])[0]; i += 2
        out = []
        for _ in range(cnt):
            f, nm, ds, na = struct.unpack('>HHHH', b[i:i + 8]); i += 8
            for _ in range(na):
                l = struct.unpack('>I', b[i + 2:i + 6])[0]; i += 6 + l
            out.append((u(nm), u(ds), f))
        return out, i
    fields, i = members(i)
    methods, i = members(i)
    return dict(access=acc, this=u(cp[this][1]), super=u(cp[sup][1]) if sup else None, fields=fields, methods=methods)


if __name__ == '__main__':
    for p in sys.argv[1:]:
        c = parse(open(p, 'rb').read())
        print(p, hex(c['access']), c['access'], c['this'], c['super'])
        for f in c['fields']:
            print('  F', f)
        for m in c['methods']:
            print('  M', m)
