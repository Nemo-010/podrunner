#!/bin/sh
# SPDX-License-Identifier: 0BSD
# The test of the installer (T-202) against a local fixture: small fake
# assets with their own pins, served from a directory (file:// of
# PODRUNNER_MIRROR). No network. The cases: the pins of the tree agree
# with images/*.toml, and a changed pin fails; a copy with no pins of a
# release refuses; a fresh install; a second run that downloads nothing
# (its mirror is empty); a corrupt asset fails by name and leaves no file;
# a changed binary is replaced; a file of the user stays; the offline
# bundle, as an archive and from its directory; --full.
#
#   sh scripts/test-install.sh
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
t=$(mktemp -d)
trap 'rm -rf "$t"' EXIT
pass=0
ok() {
    pass=$((pass + 1))
    echo "ok $pass $*"
}
fail() {
    echo "FAIL $*" >&2
    [ ! -f "$t/out" ] || sed 's/^/  | /' "$t/out" >&2
    exit 1
}
case $(uname -m) in
    x86_64 | amd64) arch=x86_64 rarch=x64 ;;
    aarch64 | arm64) arch=aarch64 rarch=arm64 ;;
    *) echo "test-install: skip: no release for $(uname -m)"; exit 0 ;;
esac
other=aarch64
[ "$arch" = x86_64 ] || other=x86_64

# 1. The pins of the tree agree with images/*.toml; a changed pin fails.
python3 -I "$root/scripts/install-pins.py" --check "$root/scripts/install.sh" > "$t/out" || fail "the pins of the tree"
sed 's/^PODVM_VERSION=.*/PODVM_VERSION="0.0.1"/' "$root/scripts/install.sh" > "$t/planted.sh"
if python3 -I "$root/scripts/install-pins.py" --check "$t/planted.sh" > "$t/out"; then fail "a planted pin passed the check"; fi
grep -q 'want PODVM_VERSION=' "$t/out" || fail "the check does not name the pin"
ok "the pins of the tree agree; a changed pin fails"

# The fixture: fake binaries, a runner tarball and image archives.
a=$t/assets
mkdir -p "$a" "$t/images/runner" "$t/log"
cat > "$a/podrunner-$arch-unknown-linux-musl" <<'EOF'
#!/bin/sh
case "$1 ${2:-}" in
    "doctor --run") echo "ok   workflow: $3" ;;
    "doctor ") echo "kvm: no"; echo "probe unshare(CLONE_NEWUSER): ok"; echo "missing: --config: no unit kind" ;;
    *) echo "podrunner 9.9.9-test" ;;
esac
EOF
# The fake podvm keeps one file for each loaded digest, and logs each load.
cat > "$a/podvm-$arch" <<EOF
#!/bin/sh
set -eu
case \$1 in
    load)
        d=\$(tar -xOf "\$3" index.json | sed 's/.*"digest": *"\\(sha256:[0-9a-f]*\\)".*/\\1/')
        mkdir -p "\$PODVM_STORE" && touch "\$PODVM_STORE/\$d" && echo "\$d" >> "$t/log/loads" ;;
    inspect) test -f "\$PODVM_STORE/\${2#runner@}" ;;
    verify) true ;;
    *) echo "podvm 0.0.0-test" ;;
esac
EOF
printf '#!/bin/sh\necho podnet-test\n' > "$a/podnet-$arch-unknown-linux-musl"
mkdir -p "$t/r/bin"
printf '#!/bin/sh\n' > "$t/r/run.sh"
printf 'listener\n' > "$t/r/bin/Runner.Listener"
tar -cf - -C "$t/r" . | gzip -c > "$a/actions-runner-linux-$rarch-2.0.0.tar.gz"
image() { # NAME TEXT
    mkdir -p "$t/oci/$1/blobs/sha256"
    printf '%s\n' "$2" > "$t/oci/$1/blobs/sha256/x"
    d=$(printf '%s' "$2" | sha256sum | cut -d' ' -f1)
    printf '{"schemaVersion": 2, "manifests": [{"digest": "sha256:%s", "annotations": {"org.opencontainers.image.ref.name": "runner:%s"}}]}\n' "$d" "$1" \
        > "$t/oci/$1/index.json"
    (cd "$t/oci/$1" && tar -cf "$a/$1" index.json blobs/sha256/x)
}
image "runner-$arch.tar" base
image "runner-full-$arch.tar" full
# The other architecture: a release needs its binary and its image.
cp "$a/podrunner-$arch-unknown-linux-musl" "$a/podrunner-$other-unknown-linux-musl"
cp "$a/runner-$arch.tar" "$a/runner-$other.tar"
sum() { sha256sum < "$1" | cut -d' ' -f1; }
pins() { # FILE NAME VERSION
    printf 'version = "%s"\n\n[sha256]\n%s = "%s"\n%s = "%s"\n' "$3" "$arch" "$(sum "$a/$2")" "$other" "$(printf '%064d' 0)" > "$1"
}
pins "$t/images/podvm.toml" "podvm-$arch" 0.0.0-test
pins "$t/images/podnet.toml" "podnet-$arch-unknown-linux-musl" 0.0.0-test
pins "$t/images/runner/runner.toml" "actions-runner-linux-$rarch-2.0.0.tar.gz" 2.0.0
installer=$t/podrunner-install.sh
python3 -I "$root/scripts/install-pins.py" --images "$t/images" --release 9.9.9-test --assets "$a" \
    "$root/scripts/install.sh" --out "$installer" > /dev/null
mkdir -p "$t/empty"

# run NAME [ARGS]: the installer in the home $t/NAME, with the mirror
# $mirror, its output in $t/out.
mirror=file://$a
run() {
    h=$t/$1
    shift
    mkdir -p "$h"
    env -u XDG_DATA_HOME -u XDG_CONFIG_HOME HOME="$h" PATH="/usr/bin:/bin" PODRUNNER_MIRROR="$mirror" \
        sh "${script:-$installer}" "$@" > "$t/out" 2>&1
}
loads() { if [ -f "$t/log/loads" ]; then wc -l < "$t/log/loads" | tr -d ' '; else echo 0; fi; }

# 2. A copy of the tree has no pins of a release, and refuses.
if script=$root/scripts/install.sh run tree; then fail "the copy of the tree installed"; fi
grep -q 'no pins of a release' "$t/out" || fail "the copy of the tree does not say why"
ok "a copy with no pins of a release refuses"

# 3. A fresh install.
run home || fail "the fresh install"
d=$t/home/.local/share/podrunner
for f in "$t/home/.local/bin/podrunner" "$d/bin/podvm" "$d/bin/podnet" "$d/runner/2.0.0/run.sh" "$d/hello.yml"; do
    [ -f "$f" ] || fail "the fresh install has no $f"
done
[ -x "$t/home/.local/bin/podrunner" ] || fail "podrunner is not executable"
digest=sha256:$(printf base | sha256sum | cut -d' ' -f1)
[ -f "$d/images/$digest" ] || fail "the image is not in the store"
toml=$t/home/.config/podrunner/run.toml
grep -qx "image = \"runner@$digest\"" "$toml" || fail "run.toml has no image"
grep -qx "podvm = \"$d/bin/podvm\"" "$toml" || fail "run.toml has no podvm"
grep -q 'user namespaces: yes' "$t/out" || fail "no line of the user namespaces"
grep -q 'next, in the tree of your project: .* run .github/workflows/ci.yml --checkout .' "$t/out" || fail "no next command"
[ -z "$(find "$t/home" -name '*.part')" ] || fail "a partial file stays"
[ "$(loads)" = 1 ] || fail "the image was loaded $(loads) times"
ok "a fresh install"

# 4. A second run with an empty mirror: it downloads nothing.
mirror=file://$t/empty
run home || fail "the second run"
grep -q 'nothing was downloaded' "$t/out" || fail "the second run downloaded"
! grep -q '^podrunner-install: fetch' "$t/out" || fail "the second run fetched"
[ "$(loads)" = 1 ] || fail "the second run loaded the image again"
ok "a second run downloads nothing"

# 5. A changed binary and a partial download are replaced, not used.
mirror=file://$a
echo changed >> "$d/bin/podvm"
echo partial > "$d/bin/podnet.part"
run home || fail "the repair"
grep -q "fetch   podvm-$arch" "$t/out" || fail "the changed podvm was not fetched again"
[ "$(sum "$d/bin/podvm")" = "$(sum "$a/podvm-$arch")" ] || fail "podvm was not replaced"
[ ! -f "$d/bin/podnet.part" ] || fail "the partial file stays"
ok "a changed binary is replaced"

# 6. A corrupt asset fails by name, and leaves no file in its place.
cp -R "$a" "$t/bad"
echo corrupt >> "$t/bad/podvm-$arch"
mirror=file://$t/bad
if run fresh-bad; then fail "a corrupt asset passed"; fi
grep -q "error: podvm-$arch: the SHA-256 is .*, not the pin" "$t/out" || fail "the corrupt asset is not named"
[ ! -e "$t/fresh-bad/.local/share/podrunner/bin/podvm" ] || fail "the corrupt podvm is in place"
[ -z "$(find "$t/fresh-bad" -name '*.part')" ] || fail "a partial file of the corrupt asset stays"
ok "a corrupt asset fails by name"

# 7. A run.toml of the user stays.
mirror=file://$a
mkdir -p "$t/mine/.config/podrunner"
echo 'podvm = "/my/podvm"' > "$t/mine/.config/podrunner/run.toml"
run mine || fail "the install with a run.toml of the user"
grep -qx 'podvm = "/my/podvm"' "$t/mine/.config/podrunner/run.toml" || fail "the file of the user changed"
[ -f "$t/mine/.config/podrunner/run.toml.podrunner-install" ] || fail "no keys of the install beside it"
ok "a run.toml of the user stays"

# 8. The offline bundle: no mirror reaches anything.
mirror=file://$t/empty
sh "$root/scripts/make-bundle.sh" "$installer" "$arch" "$a" "$t/dist" > /dev/null
(cd "$t/dist" && sha256sum -c "podrunner-bundle-$arch.tar.gz.sha256" > /dev/null) || fail "the sum of the bundle"
run offline --bundle "$t/dist/podrunner-bundle-$arch.tar.gz" || fail "the install from the bundle archive"
grep -q 'fetch' "$t/out" && fail "the bundle install fetched"
[ -f "$t/offline/.local/share/podrunner/runner/2.0.0/run.sh" ] || fail "the bundle install has no runner"
[ ! -d "$t/offline/.local/share/podrunner/downloads/bundle" ] || fail "the unpacked bundle stays"
ok "the offline bundle, as an archive"

mkdir -p "$t/unpacked"
gzip -dc < "$t/dist/podrunner-bundle-$arch.tar.gz" | tar -xf - -C "$t/unpacked"
b=$t/unpacked/podrunner-bundle-9.9.9-test-$arch
(cd "$b" && sha256sum -c SHA256SUMS > /dev/null) || fail "the SHA256SUMS of the bundle"
script=$b/podrunner-install.sh run offline2 --prefix "$t/offline2/opt" || fail "the install from the bundle directory"
[ -x "$t/offline2/opt/bin/podrunner" ] || fail "--prefix: no podrunner in PREFIX/bin"
[ -f "$t/offline2/opt/share/podrunner/bin/podvm" ] || fail "--prefix: no podvm in PREFIX/share/podrunner"
if script=$b/podrunner-install.sh run offline3 --full; then fail "--full with no full image in the bundle passed"; fi
grep -q "runner-full-$arch.tar: not in the bundle" "$t/out" || fail "--full does not name the missing image"
cp "$a/runner-full-$arch.tar" "$b/"
script=$b/podrunner-install.sh run offline3 --full || fail "--full from the bundle"
full=sha256:$(printf full | sha256sum | cut -d' ' -f1)
grep -qx "image = \"runner@$full\"" "$t/offline3/.config/podrunner/run.toml" || fail "--full: run.toml has not the full image"
ok "the offline bundle, from its directory, with --prefix and --full"

# 9. The scripts pass shellcheck, when it is there.
if command -v shellcheck > /dev/null 2>&1; then
    shellcheck "$root/scripts/install.sh" "$root/scripts/make-bundle.sh" "$root/scripts/test-install.sh" || fail "shellcheck"
    ok "shellcheck"
fi
echo "test-install: $pass passed"
