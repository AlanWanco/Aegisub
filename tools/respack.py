#!/usr/bin/env python3

import sys, os
from collections import defaultdict

manifestfile, cppfile, hfile, sourcepath = sys.argv[1:]

with open(manifestfile, 'r') as manifest:
    files = dict(
    x.strip().split('|') if '|' in x else (x.strip(), None)
    for x in manifest.readlines()
    if x.strip() and not x.lstrip().startswith('#')
)  # type: ignore

buildpath = os.path.split(cppfile)[0]

for k in files:
    if files[k] == '':
        files[k] = None

    if files[k] != None:
        if not os.path.isfile(files[k]):
            print("{}: Failed to open '{}'".format(manifestfile, k))
            sys.exit(1)
        continue

    sf = os.path.join(sourcepath, k)
    bf = os.path.join(buildpath, k)

    if os.path.isfile(sf):
        files[k] = sf
    elif os.path.isfile(bf):
        files[k] = bf
    else:
        print("{}: Failed to open '{}'".format(manifestfile, k))
        sys.exit(1)

with open(cppfile, 'w') as cpp:
    cpp.write('#include "libresrc.h"\n')
    with open(hfile, 'w') as h:
        h.write('#pragma once\n')
        groups = defaultdict(list)

        for k in files:
            with open(files[k], 'rb') as f:
                data = [str(int(x)) for x in f.read()]

            datastr = ','.join(data)
            name = os.path.splitext(os.path.basename(k))[0]
            cpp.write('const unsigned char {}[] = {{{}}};\n'.format(name, datastr))
            h.write('extern const unsigned char {}[{}];\n'.format(name, len(data)))

            parts = name.rsplit('_', 1)
            if len(parts) == 2 and parts[1].isnumeric():
                groups[parts[0]].append(name)

        if groups:
            h.write('\n')

        for group in sorted(groups):
            h.write('const LibresrcBlob {}[] = {{{}}};\n'.format(group, ', '.join('{{{}, sizeof({}), {}}}'.format(file, file, file.rsplit('_', 1)[1]) for file in groups[group])))
