import os
from zipfile import ZipFile, ZIP_DEFLATED

DST_PATH = 'strafe-jump-trainer-{}.zip'.format(os.environ['TRAVIS_TAG'])
with ZipFile(DST_PATH, mode='x', compression=ZIP_DEFLATED) as z:
    z.write('./LICENSE')
    z.write('./README.md')
    z.write('./pkg/strafe_tutorial_bg.d.ts')
    z.write('./pkg/strafe_tutorial_bg.wasm')
    z.write('./pkg/strafe_tutorial.d.ts')
    z.write('./pkg/strafe_tutorial.js')
    z.write('./static/index.html', arcname='./index.html')