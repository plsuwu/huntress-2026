import sys, zipfile, io, struct
def walk(z, path):
    for n in z.namelist():
        b = z.read(n)
        if n.endswith('.zip'): walk(zipfile.ZipFile(io.BytesIO(b)), path + n + ' -> ')
        elif n.endswith('.img'):
            gpt = b[512:520] == b'EFI PART'
            sizes = len(b)
            ents = []
            if gpt:
                lba, cnt, esz = struct.unpack_from('<QII', b, 512 + 72)
                for i in range(cnt):
                    e = b[lba*512 + i*esz: lba*512 + i*esz + 128]
                    if e[:16].strip(b'\0'): ents.append((i, struct.unpack_from('<Q', e, 32)[0]))
            bk = b[-512:-504] == b'EFI PART'
            print(path + n, sizes, 'gpt' if gpt else 'mbr', ents, 'backup@end', bk)
walk(zipfile.ZipFile(sys.argv[1]), '')
