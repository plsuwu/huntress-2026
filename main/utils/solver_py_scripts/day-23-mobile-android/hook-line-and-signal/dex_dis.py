import sys
from androguard.core.dex import DEX
d = DEX(open(sys.argv[1],'rb').read())
for c in d.get_classes():
    print("CLASS", c.get_name(), "super", c.get_superclassname())
    for f in c.get_fields():
        print("  FIELD", f.get_access_flags_string(), f.get_name(), f.get_descriptor(), f.get_init_value() and f.get_init_value().get_value())
    for m in c.get_methods():
        print("  METHOD", m.get_access_flags_string(), m.get_name(), m.get_descriptor())
        code = m.get_code()
        if not code: continue
        off = 0
        for ins in m.get_instructions():
            print("    %04x %s %s" % (off//2, ins.get_name(), ins.get_output()))
            off += ins.get_length()
