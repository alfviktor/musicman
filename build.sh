#!/bin/zsh
set -eu
cd "${0:A:h}"
app="${BUILD_DIR:-build/rust}/Musicman.app"
manifest=$(mktemp)
trap 'rm -f "$manifest"' EXIT
cargo build --release --locked --message-format=json-render-diagnostics > "$manifest"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks" "$app/Contents/Resources"
python3 - "$manifest" "$app" <<'PY'
import json, pathlib, shutil, sys
bundle=pathlib.Path(sys.argv[2])
events=[json.loads(line) for line in pathlib.Path(sys.argv[1]).read_text().splitlines()]
binary=next(e['executable'] for e in events if e['reason']=='compiler-artifact' and e['target']['name']=='musicman' and e.get('executable'))
adapter=next(p for e in events if e['reason']=='build-script-executed' and (p:=pathlib.Path(e['out_dir'])/'libPocketAudio.dylib').is_file())
shutil.copy2(binary,bundle/'Contents/MacOS/Musicman')
shutil.copy2(adapter,bundle/'Contents/Frameworks/libPocketAudio.dylib')
PY
icons="${app:h}/Musicman.iconset"
mkdir -p "$icons"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" assets/Musicman.png --out "$icons/icon_${size}x${size}.png" >/dev/null
  retina=$((size * 2))
  sips -z "$retina" "$retina" assets/Musicman.png --out "$icons/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$icons" -o "$app/Contents/Resources/Musicman.icns"
cp Info.plist "$app/Contents/Info.plist"
cp LICENSE-APACHE NOTICE "$app/Contents/Resources/"
codesign --force --sign "${SIGN_IDENTITY:--}" "$app/Contents/Frameworks/libPocketAudio.dylib"
codesign --force --sign "${SIGN_IDENTITY:--}" "$app"
codesign --verify --deep --strict "$app"
