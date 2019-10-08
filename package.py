import os, re, sys
from zipfile import ZipFile, ZIP_DEFLATED

if len(sys.argv) != 2 or not re.fullmatch(r'\d+\.\d+\.\d+', sys.argv[1]):
    print("usage: package.py <version_number>")
    sys.exit(1)

DST_PATH = 'strafe-jump-trainer-{}.zip'.format(sys.argv[1])
with ZipFile(DST_PATH, mode='x', compression=ZIP_DEFLATED) as z:
    z.write('./LICENSE')
    z.write('./README.md')
    z.write('./pkg/strafe_tutorial_bg.d.ts')
    z.write('./pkg/strafe_tutorial_bg.wasm')
    z.write('./pkg/strafe_tutorial.d.ts')
    z.write('./pkg/strafe_tutorial.js')
    z.write('./static/index.html', arcname='./index.html')